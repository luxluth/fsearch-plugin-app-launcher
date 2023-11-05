use fsearch_core::{Element, ElementBuilder, DataType, PluginAction, PluginActionType, PluginResponse};
use fsearch_core;
use xdgkit::desktop_entry::*;
use xdgkit::icon_finder;
use xdgkit::user_dirs::UserDirs;
use std::fs::{File, ReadDir};
use std::io::prelude::*;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use serde_json;
use std::time::{SystemTime, UNIX_EPOCH};


const CACHE_PATH: &str = "/tmp/fsearch_desktop_cache.json";

/// Main entry point for the application, it search for the given app name in desktop files and print the result
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return;
    }

    let query = &args[1];

    if query == "--update-cache" {
        println!("Updating cache...");
        update_desktop_cache();
        return;
    }

    let result = search(query);
    if result.is_none() {
        let response = PluginResponse {
            error: Some("No match found".to_string()),
            title: Some("Launch".to_string()),
            elements: Vec::new(),
            action: None,
            set_icon: None,
        };
        let response = fsearch_core::plugin_response_to_json(response);
        println!("{}", response);
        return;
    }
    let result = result.unwrap();

    let mut elements = Vec::<Element>::new();
    let mut i = 0;
    let mut icon = None;
    let mut exec = String::new();
    for entry in result {
        if i == 0 {
            icon = entry.icon.clone();
            exec = entry.exec.clone();
        }
        let element = entry_to_element(entry);
        elements.push(element);
        i += 1;
    }

    if elements.len() == 0 {
        let response = PluginResponse {
            title: Some("Launch".to_string()),
            error: Some("No match found".to_string()),
            elements: Vec::new(),
            action: None,
            set_icon: None,
        };
        let response = fsearch_core::plugin_response_to_json(response);
        println!("{}", response);
        return;
    }

    let response = PluginResponse {
        title: Some("Launch".to_string()),
        error: None,
        elements,
        action: Some(PluginAction {
            action: PluginActionType::Launch(exec),
            close_after_run: Some(true),
        }),
        set_icon: icon,
    };

    let response = fsearch_core::plugin_response_to_json(response);
    println!("{}", response);
}

fn entry_to_element(entry: DesktopEntryBase) -> Element {
    let icon = match entry.icon {
        Some(icon) => ElementBuilder::new(DataType::Image)
            .id("Launcher-Box-Icon")
            .image_path(icon.as_str())
            .build(),
        None => ElementBuilder::new(DataType::Image)
            .id("Launcher-Box-Icon")
            .image_path("loupe")
            .build(),
    };

    let label = ElementBuilder::new(DataType::Label)
        .id("Launcher-Box-Label")
        .text(entry.name.as_str())
        .build();

    let button = ElementBuilder::new(DataType::EventBox)
        .id("Launcher-Box")
        .children(vec![icon, label])
        .on_click(PluginAction {
            action: PluginActionType::Launch(entry.exec),
            close_after_run: Some(true),
        })
        .build();

    button
}

#[derive(Debug, Serialize, Deserialize)]
struct DesktopEntryBase {
    name: String,
    exec: String,
    icon: Option<String>,
    comment: Option<String>,
    generic_name: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedEntries {
    entries: Vec<DesktopEntryBase>,
    last_update: u64
}

fn get_icon_path(icon_name: String) -> Option<String> {
    let path = PathBuf::from(icon_name.clone());
    if path.exists() {
        return Some(icon_name);
    }

    let icon = match icon_finder::find_icon(icon_name, 48, 1) {
        Some(icon) => Some(icon),
        None => None,
    };

    if icon.is_none() {
        return None;
    }

    let icon = icon.unwrap();
    let icon = icon.to_str().unwrap().to_string();
    Some(icon)
}

fn get_desktop_entry(query: String, dir: ReadDir, max: usize) -> Vec<DesktopEntryBase> {
    let mut matches = Vec::<DesktopEntryBase>::new();

    for file in dir {
        if matches.len() >= max {
            break;
        }

        if file.is_err() {
            continue;
        }
        
        let file = file.unwrap();
        let path = file.path();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name.ends_with(".desktop") {
            let mut file = File::open(path.clone()).unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();
            let entry = DesktopEntry::read(contents);
            if entry.name.is_some() {
                let name = entry.name.clone().unwrap();
                if name.to_lowercase().contains(query.to_lowercase().as_str()) {
                    let base = DesktopEntryBase {
                        name: entry.name.unwrap(),
                        exec: entry.exec.unwrap_or("".to_string()),
                        generic_name: entry.generic_name,
                        icon: get_icon_path(entry.icon.unwrap_or("loupe".to_string())),
                        comment: entry.comment,
                    };
                    matches.push(base);
                }
            } else if entry.generic_name.is_some() {
                let name = entry.generic_name.clone().unwrap();
                if name.to_lowercase().contains(query.to_lowercase().as_str()) {
                    let base = DesktopEntryBase {
                        name: entry.name.unwrap(),
                        exec: entry.exec.unwrap_or("".to_string()),
                        generic_name: entry.generic_name,
                        icon: get_icon_path(entry.icon.unwrap_or("loupe".to_string())),
                        comment: entry.comment,
                    };
                    matches.push(base);
                }
            } else if entry.comment.is_some() {
                let name = entry.comment.clone().unwrap();
                if name.to_lowercase().contains(query.to_lowercase().as_str()) {
                    let base = DesktopEntryBase {
                        name: entry.name.unwrap(),
                        exec: entry.exec.unwrap_or("".to_string()),
                        generic_name: entry.generic_name,
                        icon: get_icon_path(entry.icon.unwrap_or("loupe".to_string())),
                        comment: entry.comment,
                    };
                    matches.push(base);
                }
            }
        }
    }


    matches
}


fn get_cache() -> Option<String> {
    let path = PathBuf::from(CACHE_PATH);
    if !path.exists() {
        update_desktop_cache();
    }

    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    Some(contents)
}

fn get_cache_entries() -> Option<CachedEntries> {
    let cache = get_cache();
    if cache.is_none() {
        return None;
    }

    let cache = cache.unwrap();
    let entries: CachedEntries = serde_json::from_str(&cache).unwrap();

    Some(entries)

}

fn has_cache() -> bool {
    let path = PathBuf::from(CACHE_PATH);
    path.exists()
}

/// Create a cache of all desktop files in the system to /tmp/fsearch_desktop_cache
fn update_desktop_cache() {

    let matches = get_matches("", 1000, false);
    let cached_entries = CachedEntries {
        entries: matches,
        last_update: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    };

    let cache = serde_json::to_string(&cached_entries).unwrap();

    let mut file = File::create(CACHE_PATH).unwrap();
    file.write_all(cache.as_bytes()).unwrap();
}

fn get_matches(
    query: &str, 
    limit: usize,
    use_cache: bool,
    ) -> Vec<DesktopEntryBase> {
    let mut matches = Vec::<DesktopEntryBase>::new();

    if use_cache && has_cache() {
        let cache_entries = get_cache_entries();
        if cache_entries.is_some() {
            let mut count = 0;
            for entry in cache_entries.unwrap().entries {
                if count >= limit {
                    break;
                }

                if entry.name.to_lowercase().contains(query.to_lowercase().as_str()) {
                    matches.push(entry);
                    count += 1;
                } else if entry.generic_name.is_some() {
                    if entry.generic_name.clone().unwrap().to_lowercase().contains(query.to_lowercase().as_str()) {
                        matches.push(entry);
                        count += 1;
                    }
                } else if entry.comment.is_some() {
                    if entry.comment.clone().unwrap().to_lowercase().contains(query.to_lowercase().as_str()) {
                        matches.push(entry);
                        count += 1;
                    }
                }
            }
        }

        return matches;
    }
    

    let user_dirs = UserDirs::new();
    let homdir = std::env::var("HOME").unwrap_or("".to_string());
    let user_desktop_files = std::fs::read_dir("/usr/share/applications");
    let user_shared_desktop_files = std::fs::read_dir("/usr/local/share/applications");
    let local_desktop_files = std::fs::read_dir(format!("{}/.local/share/applications", homdir));
    let desktop_path = user_dirs.desktop.clone().replace("$HOME", homdir.as_str());
    let on_desktop_files = std::fs::read_dir(desktop_path);
    let flatpak_desktop_files = std::fs::read_dir("/var/lib/flatpak/exports/share/applications");
    
    if user_desktop_files.is_ok() {
        let user_desktop_files = user_desktop_files.unwrap();
        let user_matches = get_desktop_entry(query.to_string(), user_desktop_files, limit);
        matches.extend(user_matches);
    }

    if user_shared_desktop_files.is_ok() {
        let user_shared_desktop_files = user_shared_desktop_files.unwrap();
        let user_shared_matches = get_desktop_entry(query.to_string(), user_shared_desktop_files, limit);
        matches.extend(user_shared_matches);
    }

    if local_desktop_files.is_ok() {
        let local_desktop_files = local_desktop_files.unwrap();
        let local_matches = get_desktop_entry(query.to_string(), local_desktop_files, limit);
        matches.extend(local_matches);
    }

    if on_desktop_files.is_ok() {
        let on_desktop_files = on_desktop_files.unwrap();
        let on_matches = get_desktop_entry("".to_string(), on_desktop_files, limit);
        matches.extend(on_matches);
    }

    if flatpak_desktop_files.is_ok() {
        let flatpak_desktop_files = flatpak_desktop_files.unwrap();
        let flatpak_matches = get_desktop_entry(query.to_string(), flatpak_desktop_files, limit);
        matches.extend(flatpak_matches);
    }

    matches.sort_by(|a, b| a.name.cmp(&b.name));
    matches
}

fn find_desktop_file(app_name: &str) -> Vec<DesktopEntryBase> {
   get_matches(app_name, 10, true) 
}

/// Search for the given app name in desktop files and print the result
fn search(query: &str) -> Option<Vec<DesktopEntryBase>> {
    let matches = find_desktop_file(query);
    Some(matches)
}

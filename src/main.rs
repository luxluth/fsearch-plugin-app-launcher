use fsearch_core::{
    DataType, Element, ElementBuilder, PluginAction, PluginActionType, PluginResponse,
};
use serde::{Deserialize, Serialize};
use std::fs::{File, ReadDir};
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use xdgkit::basedir::applications;
use xdgkit::desktop_entry::*;
use xdgkit::icon_finder;
use xdgkit::user_dirs::UserDirs;

const CACHE_PATH: &str = "/tmp/fsearch_desktop_cache.json";
const DEFAULT_ICON_PATH: &str =
    "/usr/share/icons/Adwaita/scalable/mimetypes/application-x-executable.svg";

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
    let result = result.unwrap().into_iter().enumerate();

    let mut elements = Vec::<Element>::new();
    let mut icon = None;
    let mut exec = String::new();
    for (i, entry) in result {
        if i == 0 {
            icon = entry.icon.clone();
            exec = entry.exec.clone();
        }
        let element = entry_to_element(&entry);
        elements.push(element);
    }

    if elements.is_empty() {
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

fn entry_to_element(entry: &DesktopEntryBase) -> Element {
    let icon = match &entry.icon {
        Some(icon) => ElementBuilder::new(DataType::Image)
            .id("LauncherBoxIcon")
            .image_path(icon)
            .build(),
        None => ElementBuilder::new(DataType::Image)
            .id("LauncherBoxIcon")
            .image_path(DEFAULT_ICON_PATH)
            .build(),
    };

    let label = ElementBuilder::new(DataType::Label)
        .id("LauncherBoxLabel")
        .text(&entry.name)
        .build();

    ElementBuilder::new(DataType::EventBox)
        .id("LauncherBox")
        .children(vec![icon, label])
        .on_click(PluginAction {
            action: PluginActionType::Launch(String::from(&entry.exec)),
            close_after_run: Some(true),
        })
        .build()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct DesktopEntryBase {
    name: String,
    exec: String,
    icon: Option<String>,
    comment: Option<String>,
    generic_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedEntries {
    entries: Vec<DesktopEntryBase>,
    last_update: u64,
}

fn get_icon_path(icon_name: String) -> Option<String> {
    let path = PathBuf::from(&icon_name);
    if path.exists() {
        return Some(icon_name);
    }

    if let Some(icon) = icon_finder::find_icon(icon_name, 128, 1) {
        return Some(icon.to_str().unwrap().to_string());
    };
    None
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
            let mut file = File::open(path).unwrap();
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
                        icon: get_icon_path(entry.icon.unwrap_or(DEFAULT_ICON_PATH.to_string())),
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
                        icon: get_icon_path(entry.icon.unwrap_or(DEFAULT_ICON_PATH.to_string())),
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
    let get_c = get_cache();
    let cache = get_c.as_ref()?;
    Some(serde_json::from_str(cache).unwrap())
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
        last_update: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let cache = serde_json::to_string(&cached_entries).unwrap();

    let mut file = File::create(CACHE_PATH).unwrap();
    file.write_all(cache.as_bytes()).unwrap();
}

fn get_matches(query: &str, limit: usize, use_cache: bool) -> Vec<DesktopEntryBase> {
    let query = query.to_lowercase();

    if use_cache && has_cache() {
        let mut matches = Vec::<DesktopEntryBase>::new();
        if let Some(cache_entries) = get_cache_entries() {
            let mut count = 0;
            for entry in cache_entries.entries {
                if count >= limit {
                    break;
                }

                if entry.name.to_lowercase().contains(&query) {
                    matches.push(entry);
                    count += 1;
                } else if let Some(generic_name) = &entry.generic_name {
                    if generic_name.to_lowercase().contains(&query) {
                        matches.push(entry);
                        count += 1;
                    }
                } else if let Some(comment) = &entry.comment {
                    if comment.to_lowercase().contains(&query) {
                        matches.push(entry);
                        count += 1;
                    }
                }
            }
        }

        return matches;
    }

    let matches = Arc::new(Mutex::new(Vec::<DesktopEntryBase>::new()));
    let user_dirs = Arc::new(UserDirs::new());
    let homdir = std::env::var("HOME").unwrap_or("".to_string());
    let desktop_path = user_dirs.desktop.replace("$HOME", homdir.as_str());

    let mut threads: Vec<_> = vec![];

    if let Ok(apps) = applications() {
        let apps: Vec<_> = apps.split(':').collect();
        for app_folder in apps {
            threads.push(spawn_thread(app_folder.to_string(), limit, matches.clone()))
        }
    } else {
        threads = vec![
            spawn_thread(
                "/usr/share/applications".to_string(),
                limit,
                matches.clone(),
            ),
            spawn_thread(
                "/usr/local/share/applications".to_string(),
                limit,
                matches.clone(),
            ),
            spawn_thread(
                format!("{}/.local/share/applications", homdir),
                limit,
                matches.clone(),
            ),
            spawn_thread(desktop_path, limit, matches.clone()),
            spawn_thread(
                "/var/lib/flatpak/exports/share/applications".to_string(),
                limit,
                matches.clone(),
            ),
        ];
    }

    for handle in threads {
        handle.join().unwrap();
    }

    let mut locked_matches = matches.lock().unwrap();
    locked_matches.sort_by(|a, b| a.name.cmp(&b.name));
    Vec::from(locked_matches.as_slice())
}

fn spawn_thread(
    dir: String,
    limit: usize,
    matches: Arc<Mutex<Vec<DesktopEntryBase>>>,
) -> thread::JoinHandle<()> {
    let matches_clone = Arc::clone(&matches);
    thread::spawn(move || {
        if let Ok(files) = std::fs::read_dir(dir) {
            let user_matches = get_desktop_entry("".to_string(), files, limit);
            let mut locked_matches = matches_clone.lock().unwrap();
            locked_matches.extend(user_matches);
        }
    })
}

fn find_desktop_file(app_name: &str) -> Vec<DesktopEntryBase> {
    get_matches(app_name, 10, true)
}

/// Search for the given app name in desktop files and print the result
fn search(query: &str) -> Option<Vec<DesktopEntryBase>> {
    let matches = find_desktop_file(query);
    Some(matches)
}

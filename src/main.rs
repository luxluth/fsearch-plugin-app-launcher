use fsearch_core::{Element, ElementBuilder, DataType, PluginAction, PluginActionType, PluginResponse};
use fsearch_core;
use xdgkit::desktop_entry::*;
use xdgkit::icon_finder;
use xdgkit::basedir::home;
use std::fs::{File, ReadDir};
use std::io::prelude::*;
use std::path::PathBuf;

/// Main entry point for the application, it search for the given app name in desktop files and print the result
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let query = &args[1];
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
    // TODO: add icon 
    let label = ElementBuilder::new(DataType::Label)
        .id("Launcher-Button-Label")
        .text(entry.name.as_str())
        .build();

    let comment = ElementBuilder::new(DataType::Label)
        .id("Launcher-Button-Comment")
        .text(entry.comment.unwrap_or("".to_string()).as_str())
        .build();

    let button = ElementBuilder::new(DataType::Button)
        .id("Launcher-Button")
        .children(vec![label, comment])
        .on_click(PluginAction {
            action: PluginActionType::Launch(entry.exec),
            close_after_run: Some(true),
        })
        .build();

    button
}

#[derive(Debug)]
struct DesktopEntryBase {
    name: String,
    exec: String,
    icon: Option<String>,
    comment: Option<String>,
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
                        icon: get_icon_path(entry.icon.unwrap_or("application-default-icon".to_string())),
                        comment: entry.comment,
                    };
                    matches.push(base);
                }
            } else if entry.generic_name.is_some() {
                let name = entry.generic_name.unwrap();
                if name.to_lowercase().contains(query.to_lowercase().as_str()) {
                    let base = DesktopEntryBase {
                        name: entry.name.unwrap(),
                        exec: entry.exec.unwrap_or("".to_string()),
                        icon: get_icon_path(entry.icon.unwrap_or("application-default-icon".to_string())),
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
                        icon: get_icon_path(entry.icon.unwrap_or("application-default-icon".to_string())),
                        comment: entry.comment,
                    };
                    matches.push(base);
                }
            }
        }
    }


    matches
}


fn find_desktop_file(app_name: &str) -> Option<Vec<DesktopEntryBase>> {
    // parse all desktop files to find a match in the name 
    // return the first 4 matches path
    // if no match found, return an None
    let mut matches = Vec::<DesktopEntryBase>::new();
    let homdir = home().unwrap();
    let user_desktop_files = std::fs::read_dir("/usr/share/applications");
    let local_desktop_files = std::fs::read_dir(format!("{}/.local/share/applications", homdir));
    if user_desktop_files.is_err() || local_desktop_files.is_err() {
        return None;
    }

    let user_desktop_files = user_desktop_files.unwrap();
    let local_desktop_files = local_desktop_files.unwrap();

    let user_matches = get_desktop_entry(app_name.to_string(), user_desktop_files, 10);
    let local_matches = get_desktop_entry(app_name.to_string(), local_desktop_files, 10);

    matches.extend(user_matches);
    matches.extend(local_matches);

    if matches.len() == 0 {
        return None;
    }

    Some(matches)
}

/// Search for the given app name in desktop files and print the result
fn search(query: &str) -> Option<Vec<DesktopEntryBase>> {
    let matches = find_desktop_file(query);
    if matches.is_none() {
        return None;
    }
    let matches = matches.unwrap();
    Some(matches)
}

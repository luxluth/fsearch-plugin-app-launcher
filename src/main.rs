use fsearch_core::{GtkComponent, GtkComponentBuilder, GtkComponentType, PluginAction, PluginActionType, PluginResponse};
use fsearch_core;
use fsearch_core::Align;
use xdgkit::desktop_entry::*;
use xdgkit::icon_finder;
use std::fs::File;
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
            gtk: None,
            action: None,
            set_icon: None,
        };
        let response = fsearch_core::plugin_response_to_json(response);
        println!("{}", response);
        return;
    }
    let result = result.unwrap();

    let mut content = Vec::<GtkComponent>::new();
    let mut i = 0;
    let mut icon = None;
    let mut exec = String::new();
    for entry in result {
        if i == 0 {
            icon = entry.icon.clone();
            exec = entry.exec.clone();
        }
        let gtk_entry = entry_to_gtk(entry);
        content.push(gtk_entry);
        i += 1;
    }

    let icon_path = match icon {
        Some(icon) => icon.to_str().unwrap().to_string(),
        None => "".to_string(),
    };
    
    let gtk = fsearch_core::contentify("Launch".to_string(), content);
    let response = PluginResponse {
        error: None,
        gtk: Some(vec![gtk]),
        action: Some(PluginAction {
            action: PluginActionType::Launch(exec),
            close_after_run: Some(true),
        }),
        set_icon: Some(icon_path),
    };

    let response = fsearch_core::plugin_response_to_json(response);
    println!("{}", response);
}

fn entry_to_gtk(entry: DesktopEntryBase) -> GtkComponent {
    let label = fsearch_core::new_label(entry.name, "Launcher-Button-Label".to_string(), vec![], Some(true), Some(Align::Start));
    let comment = fsearch_core::new_label(entry.comment.unwrap_or("".to_string()), "Launcher-Button-Comment".to_string(), vec![], Some(true), Some(Align::Start));

    let button = GtkComponentBuilder::new(GtkComponentType::Button)
        .id("Launcher-Button".to_string())
        .add_children(vec![label, comment])
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
    icon: Option<PathBuf>,
    comment: Option<String>,
}

fn find_desktop_file(app_name: &str) -> Option<Vec<DesktopEntryBase>> {
    // parse all desktop files to find a match in the name 
    // return the first 4 matches path
    // if no match found, return an None
    
    let mut matches = Vec::<DesktopEntryBase>::new();
    let desktop_files = std::fs::read_dir("/usr/share/applications").unwrap();
    for file in desktop_files {
        if matches.len() >= 4 {
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
                let name = entry.name.unwrap();
                if name.to_lowercase().contains(app_name.to_lowercase().as_str()) {
                    let base = DesktopEntryBase {
                        name,
                        exec: entry.exec.unwrap(),
                        icon: get_icon(entry.icon.unwrap()),
                        comment: entry.comment,
                    };
                    matches.push(base);
                }
            }
        }
    }
    if matches.len() > 0 {
        Some(matches)
    } else {
        None
    }
}


fn get_icon(icon_name: String) -> Option<PathBuf> {
    let icon = match icon_finder::find_icon(icon_name, 48, 1) {
        Some(icon) => Some(icon),
        None => None,
    };
    icon
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

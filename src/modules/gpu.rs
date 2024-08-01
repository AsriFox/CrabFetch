use core::str;
use std::{fs::{self, File, ReadDir}, io::{BufRead, BufReader, Read}, path::Path};

use serde::Deserialize;

use crate::{config_manager::Configuration, formatter::{self, CrabFetchColor}, module::Module, ModuleError};

#[derive(Clone)]
pub struct GPUInfo {
    index: Option<u8>,
    vendor: String,
    model: String,
    vram_mb: u32,
}
#[derive(Deserialize)]
pub struct GPUConfiguration {
    pub amd_accuracy: bool,
    pub ignore_disabled_gpus: bool,

    pub title: String,
    pub title_color: Option<CrabFetchColor>,
    pub title_bold: Option<bool>,
    pub title_italic: Option<bool>,
    pub separator: Option<String>,
    pub use_ibis: Option<bool>,
    pub format: String
}

impl Module for GPUInfo {
    fn new() -> GPUInfo {
        GPUInfo {
            index: None,
            vendor: "".to_string(),
            model: "".to_string(),
            vram_mb: 0
        }
    }

    fn style(&self, config: &Configuration, max_title_size: u64) -> String {
        let title_color: &CrabFetchColor = config.gpu.title_color.as_ref().unwrap_or(&config.title_color);
        let title_bold: bool = config.gpu.title_bold.unwrap_or(config.title_bold);
        let title_italic: bool = config.gpu.title_italic.unwrap_or(config.title_italic);
        let separator: &str = config.gpu.separator.as_ref().unwrap_or(&config.separator);

        let title: String = config.gpu.title.clone()
            .replace("{index}", &self.index.unwrap_or(0).to_string());
        let value: String = self.replace_color_placeholders(&self.replace_placeholders(config));

        Self::default_style(config, max_title_size, &title, title_color, title_bold, title_italic, separator, &value)
    }

    fn unknown_output(config: &Configuration, max_title_size: u64) -> String { 
        let title_color: &CrabFetchColor = config.gpu.title_color.as_ref().unwrap_or(&config.title_color);
        let title_bold: bool = config.gpu.title_bold.unwrap_or(config.title_bold);
        let title_italic: bool = config.gpu.title_italic.unwrap_or(config.title_italic);
        let separator: &str = config.gpu.separator.as_ref().unwrap_or(&config.separator);

        let title: String = config.gpu.title.clone()
            .replace("{index}", "0").to_string();

        Self::default_style(config, max_title_size, &title, title_color, title_bold, title_italic, separator, "Unknown")
    }

    fn replace_placeholders(&self, config: &Configuration) -> String {
        let use_ibis: bool = config.gpu.use_ibis.unwrap_or(config.use_ibis);

        config.gpu.format.replace("{vendor}", &self.vendor)
            .replace("{model}", &self.model)
            .replace("{vram}", &formatter::auto_format_bytes((self.vram_mb * 1000) as u64, use_ibis, 0))
    }
}
impl GPUInfo {
    pub fn set_index(&mut self, index: u8) {
        self.index = Some(index);
    }
}

pub fn get_gpus(amd_accuracy: bool, ignore_disabled_gpus: bool) -> Result<Vec<GPUInfo>, ModuleError> {
    let mut gpus: Vec<GPUInfo> = Vec::new();

    match fill_from_pcisysfile(&mut gpus, amd_accuracy, ignore_disabled_gpus) {
        Ok(_) => {},
        Err(e) => return Err(e)
    }

    Ok(gpus)
}

fn fill_from_pcisysfile(gpus: &mut Vec<GPUInfo>, amd_accuracy: bool, ignore_disabled: bool) -> Result<(), ModuleError> {
    // This scans /sys/bus/pci/devices/ and checks the class to find the first display adapter it
    // can
    // This needs expanded at a later date
    //
    // This uses pci.ids inside pciutils to identify devices:
    // https://man.archlinux.org/man/core/pciutils/pci.ids.5.en
    //
    // To prevent having to relicence to GPL I don't distribute a copy of this, and simply have to
    // rely on the host's copy. Problem then becomes that different distros seem to place this file
    // in different places
    // I'll try to find it in as many places as possible but ultimately can't cover every place. If
    // you know the places, make a PR/Issue and i'll add it in. Fucking hate licences that work
    // like this but oh well.

    let dir: ReadDir = match fs::read_dir("/sys/bus/pci/devices") {
        Ok(r) => r,
        Err(e) => return Err(ModuleError::new("GPU", format!("Can't read from /sys/bus/pci/devices: {}", e))),
    };
    for dev_dir in dir {
        // This does the following;
        // Checks "class" for a HEX value that begins with 0x03
        // (https://github.com/torvalds/linux/blob/master/include/linux/pci_ids.h#L38)
        // It then parses from "vendor" "device" and "mem_info_vram_total" to get all the info it
        // needs
        let d = match dev_dir {
            Ok(r) => r,
            Err(e) => return Err(ModuleError::new("GPU", format!("Failed to open directory: {}", e))),
        };
        // println!("{}", d.path().to_str().unwrap());
        let mut class_file: File = match File::open(d.path().join("class")) {
            Ok(r) => r,
            Err(e) => return Err(ModuleError::new("GPU", format!("Failed to open file {}: {}", d.path().join("class").to_str().unwrap(), e))),
        };
        let mut contents: String = String::new();
        match class_file.read_to_string(&mut contents) {
            Ok(_) => {},
            Err(e) => return Err(ModuleError::new("GPU", format!("Can't read from file: {}", e))),
        }

        if !contents.starts_with("0x03") {
            // Not a display device
            // And yes, I'm doing this check with a string instead of parsing it w/ a AND fuck you.
            continue
        }

        if ignore_disabled {
            let mut enable_file: File = match File::open(d.path().join("enable")) {
                Ok(r) => r,
                Err(e) => return Err(ModuleError::new("GPU", format!("Failed to open file {}: {}", d.path().join("enable").to_str().unwrap(), e))),
            };
            let mut enable_str: String = String::new();
            match enable_file.read_to_string(&mut enable_str) {
                Ok(_) => {},
                Err(e) => return Err(ModuleError::new("GPU", format!("Can't read from file: {}", e))),
            }
            if enable_str.trim() == "0" {
                continue;
            }
        }

        // Vendor/Device
        let mut vendor_file: File = match File::open(d.path().join("vendor")) {
            Ok(r) => r,
            Err(e) => return Err(ModuleError::new("GPU", format!("Failed to open file {}: {}", d.path().join("vendor").to_str().unwrap(), e))),
        };
        let mut vendor_str: String = String::new();
        match vendor_file.read_to_string(&mut vendor_str) {
            Ok(_) => {},
            Err(e) => return Err(ModuleError::new("GPU", format!("Can't read from file: {}", e))),
        }
        let vendor: &str = vendor_str[2..].trim();

        let mut device_file: File = match File::open(d.path().join("device")) {
            Ok(r) => r,
            Err(e) => return Err(ModuleError::new("GPU", format!("Failed to open file {}: {}", d.path().join("device").to_str().unwrap(), e))),
        };
        let mut device_str: String = String::new();
        match device_file.read_to_string(&mut device_str) {
            Ok(_) => {},
            Err(e) => return Err(ModuleError::new("GPU", format!("Can't read from file: {}", e))),
        }
        let device: &str = device_str[2..].trim();
        let dev_data: (String, String) = match search_pci_ids(vendor, device) {
            Ok(r) => r,
            Err(e) => return Err(e)
        };

        let mut gpu: GPUInfo = GPUInfo::new();
        gpu.vendor = dev_data.0;
        if vendor == "1002" && amd_accuracy { // AMD
            gpu.model = match search_amd_model(device)? {
                Some(r) => r,
                None => dev_data.1,
            };
        } else {
            gpu.model = dev_data.1;
        }

        // Finally, Vram
        if let Ok(mut r) = File::open(d.path().join("mem_info_vram_total")) {
            let mut vram_str: String = String::new();
            match r.read_to_string(&mut vram_str) {
                Ok(_) => {},
                Err(e) => return Err(ModuleError::new("GPU", format!("Can't read from file: {}", e))),
            }
            gpu.vram_mb = (vram_str.trim().parse::<u64>().unwrap() / 1024 / 1024) as u32;
        }

        gpus.push(gpu);
    }

    Ok(())
}
fn search_pci_ids(vendor: &str, device: &str) -> Result<(String, String), ModuleError> {
    // Search all known locations
    // /usr/share/hwdata/pci.ids
    let mut ids_path: Option<&str> = None;
    if Path::new("/usr/share/hwdata/pci.ids").exists() {
        ids_path = Some("/usr/share/hwdata/pci.ids");
    } else if Path::new("/usr/share/misc/pci.ids").exists() {
        ids_path = Some("/usr/share/misc/pci.ids");
    }

    if ids_path.is_none() {
        return Err(ModuleError::new("GPU", "Could not find an appropriate path for getting PCI ID info.".to_string()));
    }

    let file: File = match File::open(ids_path.unwrap()) {
        Ok(r) => r,
        Err(e) => return Err(ModuleError::new("GPU", format!("Can't read from {} - {}", ids_path.unwrap(), e))),
    };
    let buffer: BufReader<File> = BufReader::new(file);

    // parsing this file is weird
    let mut vendor_result: String = String::new();
    let mut device_result: String = String::new();
    // Find the vendor ID + device in the list
    let vendor_term: String = String::from(vendor);
    let dev_term: String = String::from('\t') + device;
    let mut in_vendor: bool = false;
    for line in buffer.lines() { // NOTE: Looping here alone takes 1.7ms - This needs to be reduced
        if line.is_err() {
            continue;
        }
        let line: String = line.unwrap();

        if line.trim().starts_with('#') {
            continue
        }

        if in_vendor && line.chars().next().is_some() {
            in_vendor = line.chars().next().unwrap().is_whitespace();
            if !in_vendor {
                // Assume we missed it
                break
            }
        }

        if line.starts_with(&vendor_term) && vendor_result.is_empty() {
            // Assume the first hit of this is our full vendor name
            vendor_result = line[vendor_term.len()..].trim().to_string();
            in_vendor = true;
        } else if line.starts_with(&dev_term) && in_vendor {
            // And here's the device name
            device_result = line[dev_term.len()..].trim().to_string();
            break
        }
    }

    if device_result.is_empty() {
        device_result += device;
    }

    Ok((vendor_result.to_string(), device_result.to_string()))
}
// TODO: Revision ID searching too
fn search_amd_model(device: &str) -> Result<Option<String>, ModuleError> {
    let mut ids_path: Option<&str> = None;
    if Path::new("/usr/share/libdrm/amdgpu.ids").exists() {
        ids_path = Some("/usr/share/libdrm/amdgpu.ids");
    }
    if ids_path.is_none() {
        return Err(ModuleError::new("GPU", "Could not find an appropriate path for getting AMD PCI ID info.".to_string()));
    }

    let file: File = match File::open(ids_path.unwrap()) {
        Ok(r) => r,
        Err(e) => return Err(ModuleError::new("GPU", format!("Can't read from {} - {}", ids_path.unwrap(), e))),
    };
    let buffer: BufReader<File> = BufReader::new(file);

    let mut device_result: String = String::new();
    let dev_term: String = device.to_lowercase().to_string();
    for line in buffer.lines() { 
        if line.is_err() {
            continue;
        }
        let line: String = line.unwrap();

        if line.trim().starts_with('#') {
            continue
        }

        if line.to_lowercase().starts_with(&dev_term) {
            device_result = line.split('\t').nth(2).unwrap().trim().to_string();
            break
        }
    }

    if device_result.is_empty() {
        return Ok(None)
    }

    Ok(Some(device_result.to_string()))
}

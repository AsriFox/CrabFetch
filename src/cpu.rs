use core::str;
use std::{fmt::Display, fs::File, io::Read, path::Path};

use crate::Fetchable;

pub struct CPUInfo {
    name: String,
    cores: u16,
    threads: u16,
    current_clock: f32,
    max_clock: f32,
    temperature: f32,
}
impl Fetchable for CPUInfo {
    fn new() -> CPUInfo {
        CPUInfo {
            name: "".to_string(),
            cores: 0,
            threads: 0,
            current_clock: 0.0,
            max_clock: 0.0,
            temperature: 0.0
        }
    }
    fn format(&self, format: &str) -> String {
        format.replace("{name}", &self.name)
        .replace("{core_count}", &self.cores.to_string())
        .replace("{thread_count}", &self.threads.to_string())
        .replace("{current_clock_mhz}", &self.current_clock.to_string())
        .replace("{current_clock_ghz}", &(self.current_clock / 1000.0).to_string())
        .replace("{max_clock_mhz}", &self.max_clock.to_string())
        .replace("{max_clock_ghz}", &(self.max_clock / 1000.0).to_string())
        .replace("{temp}", &self.temperature.to_string())
    }
}
impl Display for CPUInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}c {}t) @ {}GHz [{}°C]", self.name, self.cores, self.threads, self.max_clock / 1000.0, self.temperature)
    }
}

pub fn get_cpu() -> CPUInfo {
    let mut cpu = CPUInfo::new();
    get_basic_info(&mut cpu);
    get_max_clock(&mut cpu);
    get_temperature(&mut cpu);

    cpu
}

fn get_basic_info(cpu: &mut CPUInfo) {
    // Starts by reading and parsing /proc/cpuinfo
    // This gives us the cpu name, cores, threads and current clock
    // TODO: Average the current clock so that it's not just on core 0 we're reading it
    let mut file: File = match File::open("/proc/cpuinfo") {
        Ok(r) => r,
        Err(e) => {
            // Best guess I've got is that we're not on Linux
            // In which case, L
            panic!("Can't read from /proc/cpuinfo - {}", e);
        },
    };
    let mut contents: String = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {},
        Err(e) => {
            panic!("Can't read from /proc/cpuinfo - {}", e);
        },
    }

    // Now we parse
    // Just doing one entry as the rest are kinda redundant
    let entry: &str = contents.split("\n\n").collect::<Vec<&str>>()[0];
    let lines: Vec<&str> = entry.split("\n").collect();
    for line in lines {
        if line.starts_with("model name") {
            cpu.name = line.split(": ").collect::<Vec<&str>>()[1].to_string();
        }
        if line.starts_with("cpu cores") {
            cpu.cores = match line.split(": ").collect::<Vec<&str>>()[1].parse::<u16>() {
                Ok(r) => r,
                Err(e) => {
                    println!("WARNING: Could not parse cpu cores: {}", e);
                    0
                },
            }
        }
        if line.starts_with("siblings") {
            cpu.threads = match line.split(": ").collect::<Vec<&str>>()[1].parse::<u16>() {
                Ok(r) => r,
                Err(e) => {
                    println!("WARNING: Could not parse cpu threads: {}", e);
                    0
                },
            }
        }
        if line.starts_with("cpu MHz") {
            cpu.current_clock = match line.split(": ").collect::<Vec<&str>>()[1].parse::<f32>() {
                Ok(r) => r,
                Err(e) => {
                    println!("WARNING: Could not parse current cpu frequency: {}", e);
                    0.0
                },
            }
        }
    }
}
fn get_max_clock(cpu: &mut CPUInfo) {
    // All of this is relative to /sys/devices/system/cpu/cpu0/cpufreq
    // There's 3 possible places to get the frequency in here;
    // - bios_limit - Only present if a limit is set in BIOS
    // - scaling_max_freq - The max freq set by the policy
    // - cpuinfo_max_freq - The max possible the CPU can run at uncapped
    //
    // This just takes the first of those three that are present
    //
    // Source: https://docs.kernel.org/admin-guide/pm/cpufreq.html

    let mut freq_path: Option<&str> = None;
    if Path::new("/sys/devices/system/cpu/cpu0/cpufreq/bios_limit").exists() {
        freq_path = Some("/sys/devices/system/cpu/cpu0/cpufreq/bios_limit");
    } else if Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq").exists() {
        freq_path = Some("/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq");
    } else if Path::new("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq").exists() {
        freq_path = Some("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq");
    }

    if freq_path.is_none() {
        panic!("Could not find an appropriate path for getting max CPU Frequency.");
    }

    let mut file: File = match File::open(freq_path.unwrap()) {
        Ok(r) => r,
        Err(e) => {
            panic!("Can't read from {} - {}", freq_path.unwrap(), e);
        },
    };
    let mut contents: String = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {},
        Err(e) => {
            panic!("Can't read from {} - {}", freq_path.unwrap(), e);
        },
    }

    match contents.trim().parse::<f32>() {
        Ok(r) => {
            cpu.max_clock = r / 1000.0
        },
        Err(_) => {}
    };
}
fn get_temperature(cpu: &mut CPUInfo) {
    // To get the temp I'm reading from /sys/class/thermal/thermal_zone0/temp
    // Not sure if this is a consistent way to get the CPU temperature, but it will do for now.

    let mut file: File = match File::open("/sys/class/thermal/thermal_zone0/temp") {
        Ok(r) => r,
        Err(e) => {
            panic!("Can't read from /sys/class/thermal/thermal_zone0/temp - {}", e);
        },
    };
    let mut contents: String = String::new();
    match file.read_to_string(&mut contents) {
        Ok(_) => {},
        Err(e) => {
            panic!("Can't read from /sys/class/thermal/thermal_zone0/temp - {}", e);
        },
    }

    match contents.trim().parse::<f32>() {
        Ok(r) => {
            cpu.temperature = r / 1000.0;
        },
        Err(_) => {}
    };
}

extern crate serde_json;
extern crate reqwest;
extern crate tar;
extern crate libc;
extern crate sha2;
mod hybris;

//use std::fs::File;
//use tar::Archive;
use serde_json::Value;
use std::path::Path;
use std::fs;
use std::env;
use std::fs::File;
use std::io::prelude::*;

fn get_cache_path() -> &'static Path {
    let path = Path::new("/cache/afirmflasher");

    if !path.exists() {
        fs::create_dir(path).unwrap();
    }

    path
}

fn get_json_file(url: &str) -> Result<Value, reqwest::Error> {
    let req: Value = reqwest::get(url)?
        .json()?;
    Ok(req)
}

fn get_devices() -> Result<Value, reqwest::Error> {
    get_json_file("http://cdimage.ubports.com/devices/blobs/devices.json")
}

fn get_device() -> String {
    hybris::properties::get("ro.product.name", "")
}

fn download_file(url: &str, name: &str) -> Result<(), reqwest::Error> {
    let mut resp = reqwest::get(url)?;
    let mut buf: Vec<u8> = vec![];
    resp.copy_to(&mut buf)?;

    File::open(get_cache_path().join(name)).unwrap().write(&buf).unwrap();

    println!("Downloaded {:?} into {:?}", url, get_cache_path().join(name));

    Ok(())
}

fn write_to_partition(file: &str, partition: &str) -> Result<(), std::io::Error> {
    let mut bytes = Vec::new();
    File::open(get_cache_path().join(file))?.read_to_end(&mut bytes)?;
    File::open(partition)?.write(&bytes);

    println!("Write {:?} into {:?}", file, partition);

    Ok(())
}

/* Unused for the time beeing, will use later
fn read_partition_json(path: PathBuf) -> Result<Value, Box<std::error::Error>> {
    let raw_file = path.join("partitions.json");
    let file = File::open(raw_file)?;
    let json: Value = serde_json::from_reader(file)?;

    Ok(json)
}

fn extract_and_read_paritions() -> Value {
    let extracted_paritions = extract_partitions();
    read_partition_json(extracted_paritions).unwrap()
}

fn extract_partitions() -> PathBuf {
    let dir = get_cache_path();
    let tarball = dir.join("partitions.tar.xz");
    let output = extract_tarball(&tarball, dir).unwrap();

    output
}

fn extract_tarball(tarball: &Path, dest: &Path) -> Result<PathBuf, Box<std::error::Error>> {
    let out_dir = dest.clone().join("output");
    let file = File::open(tarball)?;
    let lz = LzmaReader::new_decompressor(file)?;

    let mut ar = Archive::new(lz);
    ar.unpack(out_dir.clone())?;

    println!("OK");

    Ok(out_dir)
}
*/

fn checksum(file: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::default();

    let mut bytes = Vec::new();
    File::open(get_cache_path().join(file)).unwrap().read_to_end(&mut bytes).unwrap();
    hasher.input(bytes.as_ref());

    format!("{:x}", hasher.result())
}

fn check_paritions_checksums(par: &Vec<Value>, url: &str, no_flash: bool) {
    for val in par.iter() {
        if val.is_object() {
            let valobj = val.as_object().unwrap();
            if valobj.contains_key("checksum")
            || valobj.contains_key("partition")
            || valobj.contains_key("file") {
                let partition = valobj["partition"].as_str().unwrap();
                let file = valobj["file"].as_str().unwrap();
                let checksum_str = valobj["checksum"].as_str().unwrap();
                if checksum(partition) == checksum_str {
                    println!("partition {:?} is up to date", partition);
                    continue;
                }
                if no_flash {
                    println!("partition {:?} is NOT up to date", partition);
                    continue;
                }

                println!("Downloading partition {:?}", partition);
                download_file(&format!("{}/{}", url, file), file).unwrap();
                println!("Flashing partition {:?}", partition);
                write_to_partition(file, partition).unwrap();

                if checksum(partition) != checksum_str {
                    println!("ERROR FILE DOES NOT MACH PARITION AFTER FLASH");
                }
            }
        }
    }
}

fn flash_partition_if_newer(no_flash: bool) {
    let devices_r = get_devices().unwrap();
    let devices_v = devices_r.as_object().unwrap();
    if !devices_v["devices"].is_object() {
        println!("Devices json is not an object!");
        return;
    }

    let devices = devices_v["devices"].as_object().unwrap();
    let device = get_device();

    if !devices.contains_key(&device) {
        println!("Could not find device {:?} in devices json", device);
        return;
    }

    let url = devices[&device].as_str().unwrap();

    let paritions_obj = get_json_file(&format!("{}/partitions.json", url)).unwrap();
    if !paritions_obj.is_object()
    || !paritions_obj.as_object().unwrap().contains_key("partitions")
    || !paritions_obj.as_object().unwrap()["partitions"].is_array() {
        println!("paritions.json is not valid");
    }
    let paritions_array = paritions_obj.as_object().unwrap()["partitions"].as_array().unwrap();

    check_paritions_checksums(paritions_array, url, no_flash);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("Device is: {:?}", get_device());
    if args.len() > 1 && &args[1] == "-d" {
        println!("Checking paritions");
        flash_partition_if_newer(true);
        return;
    }
    flash_partition_if_newer(false);
}

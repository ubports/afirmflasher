// Copyright (c) 2018 Marius Gripsgard <marius@ubports.com>
//
// GNU GENERAL PUBLIC LICENSE
//    Version 3, 29 June 2007
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

#[macro_use]
extern crate serde_json;

extern crate reqwest;
extern crate tar;
extern crate libc;
extern crate sha2;
extern crate dbus;
mod hybris;

//use std::fs::File;
//use tar::Archive;
use serde_json::Value;
use std::path::Path;
use std::fs;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use dbus::{Connection, BusType, NameFlag};
use dbus::tree::Factory;
use std::sync::{Arc, Mutex};

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

    File::create(get_cache_path().join(name)).unwrap().write(&buf).unwrap();

    println!("Downloaded {:?} into {:?}", url, get_cache_path().join(name));

    Ok(())
}

fn download_file_quiet(url: &str, name: &str) -> Result<(), reqwest::Error> {
    let mut resp = reqwest::get(url)?;
    let mut buf: Vec<u8> = vec![];
    resp.copy_to(&mut buf)?;

    File::create(get_cache_path().join(name)).unwrap().write(&buf).unwrap();

    Ok(())
}

fn write_to_partition(file: &str, partition: &str) -> Result<(), std::io::Error> {
    let mut bytes = Vec::new();
    File::open(get_cache_path().join(file))?.read_to_end(&mut bytes)?;
    File::create(partition)?.write_all(&bytes).unwrap();

    println!("Write {:?} into {:?}", file, partition);

    Ok(())
}

fn write_to_partition_quiet(file: &str, partition: &str) -> Result<(), std::io::Error> {
    let mut bytes = Vec::new();
    File::open(get_cache_path().join(file))?.read_to_end(&mut bytes)?;
    File::create(partition)?.write_all(&bytes).unwrap();

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

fn for_each_partition<F>(par: Vec<Value>, func: F) -> Vec<Value>  where F: Fn(&str, &str, &str, Value, &mut Vec<Value>){
    let mut vec: Vec<Value> = Vec::new();
    for val in par.iter() {
        if val.is_object() {
            let valobj = val.as_object().unwrap();
            if valobj.contains_key("checksum")
            || valobj.contains_key("partition")
            || valobj.contains_key("file") {
                let partition = valobj["partition"].as_str().unwrap();
                let file = valobj["file"].as_str().unwrap();
                let checksum_str = valobj["checksum"].as_str().unwrap();
                func(partition, file, checksum_str, val.to_owned(), &mut vec);
            }
        }
    }

    vec
}

fn print_paritions(par: Vec<Value>) {
    for_each_partition(par, |partition, _file, _checksum_str, _value, _vec| {
        println!("Partition {:?} is not up to date!", partition);
    });
}


fn download_and_flash_paritions(par: Vec<Value>, url: String) {
    for_each_partition(par, |partition, file, checksum_str, _value, _vec| {
        println!("Downloading partition {:?}", partition);
        download_file(&format!("{}/{}", url, file), file).unwrap();
        println!("Flashing partition {:?}", partition);
        write_to_partition(file, partition).unwrap();

        if checksum(partition) != checksum_str {
            println!("ERROR FILE DOES NOT MACH PARITION AFTER FLASH");
        }
    });
}

fn download_and_flash_paritions_quiet(par: Vec<Value>, url: String) -> Vec<Value> {
    for_each_partition(par, |partition, file, checksum_str, value, vec| {
        download_file_quiet(&format!("{}/{}", url, file), file).unwrap();
        write_to_partition_quiet(file, partition).unwrap();

        if checksum(partition) != checksum_str {
            vec.push(value);
        }
    })
}

fn check_paritions_checksums(par: Vec<Value>) -> Vec<Value> {
    let rvec = for_each_partition(par, |partition, _file, checksum_str, value, vec| {
        if checksum(partition) == checksum_str {
            println!("partition {:?} is up to date", partition);
            return;
        }

        vec.push(value);
    });

    rvec
}

fn check_paritions_checksums_quiet(par: Vec<Value>) -> Vec<Value> {
    let rvec = for_each_partition(par, |partition, _file, checksum_str, value, vec| {
        if checksum(partition) == checksum_str {
            return;
        }

        vec.push(value);
    });

    rvec
}

fn get_device_url() -> Result<String, &'static str> {
    let devices_r = get_devices().unwrap();
    let devices_v = devices_r.as_object().unwrap();
    if !devices_v["devices"].is_object() {
        //println!("Devices json is not an object!");
        return Err("Devices json is not an object!");
    }

    let devices = devices_v["devices"].as_object().unwrap();
    let device = get_device();

    if !devices.contains_key(&device) {
        //println!("Could not find device {:?} in devices json", device);
        return Err("Could not find device in devices json");
    }

    Ok(devices[&device].as_str().unwrap().to_owned())
}

fn get_device_paritions_obj(url: String) -> Result<Vec<Value>, &'static str> {
    let paritions_obj = get_json_file(&format!("{}/partitions.json", url)).unwrap();
    if !paritions_obj.is_object()
    || !paritions_obj.as_object().unwrap().contains_key("partitions")
    || !paritions_obj.as_object().unwrap()["partitions"].is_array() {
        println!("partitions.json is not valid");
        return Err("partitions.json is not valid");
    }
    let paritions_array = paritions_obj.as_object().unwrap()["partitions"].as_array().unwrap().to_owned();

    Ok(paritions_array)
}

fn flash_partition_if_newer(no_flash: bool) {
    let url = get_device_url().unwrap();
    let partitions_array = get_device_paritions_obj(url.clone()).unwrap();

    let result = check_paritions_checksums(partitions_array);
    if !no_flash {
        download_and_flash_paritions(result, url);
    } else {
        print_paritions(result);
    }
}

fn get_need_update_list() ->  Vec<Value> {
    let url = get_device_url().unwrap();
    let partitions_array = get_device_paritions_obj(url.clone()).unwrap();

    check_paritions_checksums(partitions_array)
}

/*
fn daemon() {
    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("com.ubports.afirmflasher", NameFlag::ReplaceExisting as u32).unwrap();
    let f = Factory::new_fn::<()>();
    let tree = f.tree(()).add(f.object_path("/", ()).introspectable().add(
        f.interface("com.ubports.afirmflasher", ()).add_m(
            /*
                {
                    need_update: bool,
                    paritions: array
                }
            */
            f.method("check_for_update", (), |m| {
                let val = get_need_update_list();

                Ok(vec!(m.msg.method_return().append1(val.to_string().unwrap())))
            }).inarg::<_,_>("name")
              .outarg::<&str,_>("reply")
        ).add_m(
            f.method("flash_update", (), |m| {
                let n: &str = m.msg.read1()?;
                let s = format!("Hello {}!", n);
                Ok(vec!(m.msg.method_return().append1(s)))
            }).inarg::<&str,_>("name")
              .outarg::<&str,_>("reply")
        )
    ));
    tree.set_registered(&c, true).unwrap();
    c.add_handler(tree);
    loop { c.incoming(1000).next(); }
}
*/

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && &args[1] == "-n" {
        println!("Checking paritions");
        println!("Device is: {:?}", get_device());
        flash_partition_if_newer(true);
        return;
    }
    if args.len() > 1 && &args[1] == "-jc" {
        let url = get_device_url().unwrap();
        let partitions_array = get_device_paritions_obj(url.clone()).unwrap();
        let c = json!(check_paritions_checksums_quiet(partitions_array));
        println!("{:}", c.to_string());
        return;
    }
    if args.len() > 1 && &args[1] == "-jd" {
        let url = get_device_url();
        match url {
            Ok(_) => println!("OK"),
            Err(_) => println!("ERR"),
        }
        return
    }
    if args.len() > 1 && &args[1] == "-jf" {
        let url = get_device_url().unwrap();
        let partitions_array = get_device_paritions_obj(url.clone()).unwrap();
        let upd = check_paritions_checksums_quiet(partitions_array);
        let c = json!(download_and_flash_paritions_quiet(upd, url));

        println!("{:}", c.to_string());
        return;
    }
    /*
    if args.len() > 1 && &args[1] == "-d" {
        println!("Running daemon!");
        daemon();
        return;
    }
    */
    println!("Device is: {:?}", get_device());
    flash_partition_if_newer(false);
}

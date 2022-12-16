use std::fs;
use std::io;
use std::time::SystemTime;
use itertools::Itertools;

fn main() -> io::Result<()> {
    let path = "../content/recettes";
    let entries = fs::read_dir(path)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    // sort the entries by last modification time
    let mut sorted_entries = entries.iter().map(|entry| {
        let metadata = entry.metadata()?;
        let modified_time = metadata.modified()?;
        Ok((entry, modified_time))
    }).collect::<Result<Vec<_>, io::Error>>()?;
    sorted_entries.sort_by(|a, b| a.1.cmp(&b.1));

    // read the date from the file
    let file_date_string = match fs::read_to_string("date_file.txt") {
        Ok(date_string) => date_string,
        Err(_) => "".to_string(), // if the file does not exist or there was an error reading it, use an empty string
    };

    // parse the date string from the file
    let file_date = match file_date_string.parse::<SystemTime>() {
        Ok(date) => date,
        Err(_) => SystemTime::UNIX_EPOCH, // if the date string is invalid, use the Unix epoch as the date
    };

    // get the date of the first entry
    let first_entry_date = sorted_entries[0].1;

    // only process the entries if the first entry's date is newer than the date in the file
    if first_entry_date > file_date {
        // group the sorted entries into chunks of 20
        let chunked_entries: Vec<Vec<_>> = sorted_entries.chunks(15).map(|chunk| chunk.to_vec()).collect();

        // print the file paths in each chunk
        for chunk in chunked_entries {
            println!("--- CHUNK ---");
            for entry in chunk {
                println!("{:?}", entry.0);
            }
        }

        // write the date of the first entry to the file
        fs::write("date_file.txt", first_entry_date.as_secs().to_string())?;
    }

    Ok(())
}

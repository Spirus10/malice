    use colored::*;
    use chrono::{self, Datelike, Timelike};

    pub fn good(msg: &str) {
        let current_date = chrono::offset::Local::now();
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();

        let hour = current_date.hour();
        let minute = current_date.minute();
        let second = current_date.second();

        println!(
            "{} {}, {} : {}",
            "[GOOD]".green(),
            format!("{}/{}/{}", month, day, year).blue(),
            format!("{}:{}:{}", hour, minute, second).purple(),
            format!("{}", msg)
        );
    }

        pub fn bad(msg: &str) {
            let current_date = chrono::offset::Local::now();
            let year = current_date.year();
            let month = current_date.month();
            let day = current_date.day();
    
            let hour = current_date.hour();
            let minute = current_date.minute();
            let second = current_date.second();
    
            println!(
                "{} {}, {} : {}",
                "[BAD]".red(),
                format!("{}/{}/{}", month, day, year).blue(),
                format!("{}:{}:{}", hour, minute, second).purple(),
                format!("{}", msg)
            );
        }

        pub fn info(msg: &str) {
            let current_date = chrono::offset::Local::now();
            let year = current_date.year();
            let month = current_date.month();
            let day = current_date.day();
    
            let hour = current_date.hour();
            let minute = current_date.minute();
            let second = current_date.second();
    
            println!(
                "{} {}, {} : {}",
                "[INFO]".cyan(),
                format!("{}/{}/{}", month, day, year).blue(),
                format!("{}:{}:{}", hour, minute, second).purple(),
                format!("{}", msg)
            );
    }

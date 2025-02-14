use std::{collections::HashMap, fs, io::{self, stdin, Write}, process::{self, Stdio}};
use moodle::client;
use serde_json::value::Value;
use html2text;

const BASE_URL: &str = "https://game.spengergasse.at";
const ASSIGNMENT_ID: &str = "2016";
const ZOEY_USER_ID: &str = "219";

async fn get_user_fullname(client: &mut client::MoodleClient, id: i64) -> String
{
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("field".to_string(), "id".to_string());
    params.insert("values[0]".to_string(), id.to_string());
    match client.post("core_user_get_users_by_field", &params).await {
        Ok(result) => {
            let firstname = result[0]["firstname"].as_str().unwrap_or("Firstname");
            let lastname = result[0]["lastname"].as_str().unwrap_or("Lastname");
            format!("{firstname} {lastname}")
        }
        Err(_) => "".to_string()
    }
}

fn test_submission(name: &str, code: &str)
{
    std::env::set_current_dir("submissions").unwrap();
    let dir = name.to_lowercase().replace(" ", "-");
    if !fs::exists(&dir).unwrap_or(false) {
        let _ = process::Command::new("cargo")
            .arg("new")
            .arg(&dir)
            .status();
    }
    std::env::set_current_dir(dir).unwrap();
    let _ = process::Command::new("cargo")
        .arg("add")
        .arg("rand")
        .status();
    let mut code_file = fs::File::create("src/main.rs").unwrap();
    code_file.write_all(code.as_bytes()).unwrap();
    println!("{}", name);
    let _ = process::Command::new("cargo")
        .arg("run")
        .stdout(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output();
    println!("{}", code);
    println!("{}", name);
    std::env::set_current_dir("../..").unwrap();
}

async fn grade_submission(client: &mut client::MoodleClient, user_id: i64)
{
    loop {
        println!("Which grade? (P/W/M/-)");
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        let grade = match input.trim() {
            "P" => 3,
            "W" => 2,
            "M" => 1,
            "-" => break,
            _ => continue
        };

        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("assignmentid".to_string(), ASSIGNMENT_ID.to_string());
        params.insert("userid".to_string(), user_id.to_string());
        params.insert("grade".to_string(), grade.to_string());
        params.insert("attemptnumber".to_string(), "-1".to_string());
        params.insert("addattempt".to_string(), "0".to_string());
        params.insert("workflowstate".to_string(), "released".to_string());
        params.insert("applytoall".to_string(), "0".to_string());
        match client.post("mod_assign_save_grade", &params).await {
            Ok(result) => {
                println!("{:#?}", result);
            },
            Err(e) => {
                println!("ERROR grade_submission: {e}");
            }
        }
        break
    }
}

async fn check_submission(client: &mut client::MoodleClient, submission: &Value)
{
    if submission["status"].as_str().unwrap_or("") != "submitted" {
        return;
    }
    println!("{:#?}", submission);
    let user_id = submission["userid"].as_i64().unwrap_or(-1);
    if user_id < 0 {
        println!("Error checking submission! Could not find submissionid or userid.");
        return;
    }
    let student_name = get_user_fullname(client, submission["userid"].as_i64().unwrap_or(-1)).await;
    let submission_html = submission["plugins"][0]["editorfields"][0]["text"].as_str().unwrap_or("");
    let submission_text = html2text::from_read(submission_html.as_bytes(), 200).unwrap();
    let submission_text_trim = submission_text.trim_matches(&['`','\n']);
    test_submission(&student_name, submission_text_trim);
    grade_submission(client, user_id).await;
}

async fn checker(client: &mut client::MoodleClient)
{
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("assignmentids[0]".to_string(), ASSIGNMENT_ID.to_string());
    
    match client.post("mod_assign_get_submissions", &params).await {
        Ok(result) => {
            match result["assignments"][0]["submissions"].as_array() {
                Some(submissions) => {
                    for submission in submissions {
                        check_submission(client, submission).await;
                    }
                },
                None => {
                    println!("No submissions found for assignment {}.", ASSIGNMENT_ID);
                }
            }
            //println!("{:#?}", result["assignments"][0]["submissions"][0]);
        },
        Err(e) => {
            println!("Error fetching submissions: {e}");
        }
        }
}

#[tokio::main]
async fn main() {
    let file = fs::File::open("login.json").expect("Could not open JSON file with login information!");
    let reader = io::BufReader::new(file);
    let login_json:Value = serde_json::from_reader(reader).unwrap();
    let username = login_json["username"].as_str().unwrap();
    let password = login_json["password"].as_str().unwrap();
    match client::login(BASE_URL, username, password).await {
        Ok(token) => {
            let mut client = client::MoodleClient::new(BASE_URL, &token);
            checker(&mut client).await;
        },
        Err(e) => {
            println!("Authentication failed: {}", e);
        }
    }
}

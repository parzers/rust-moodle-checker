use std::{collections::HashMap, fs, io::{self, stdin, Write}, process::{self, Stdio}, sync::{self, atomic::AtomicU32, Arc, Mutex}};
use moodle::client;
use serde_json::value::Value;
use html2text;
use winapi::um::wincon::GenerateConsoleCtrlEvent;
use std::sync::atomic::{Ordering, AtomicI64};

const BASE_URL: &str = "https://game.spengergasse.at";
const COURSE_ID: &str = "36";
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

fn test_submission(name: &str, code: &str, pid: &Arc<AtomicU32>)
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
    let child = process::Command::new("cargo")
        .arg("run")
        .stdout(Stdio::inherit())
        .stderr(Stdio::null())
        .stdin(Stdio::inherit())
        .spawn()
        .unwrap();
    pid.store(child.id(), Ordering::Relaxed);
    println!("{}", code);
    child.wait_with_output().expect("Failed to wait for child process");
    pid.store(0, Ordering::Relaxed);
    println!("{}", name);
    std::env::set_current_dir("../..").unwrap();
}

async fn grade_submission(client: &mut client::MoodleClient, user_id: i64, assignment_id: i64)
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
        params.insert("assignmentid".to_string(), assignment_id.to_string());
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

async fn check_submission(client: &mut client::MoodleClient, submission: &Value, assignment_id: i64, pid: &Arc<AtomicU32>)
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
    test_submission(&student_name, submission_text_trim, pid);
    grade_submission(client, user_id, assignment_id).await;
}

async fn checker(client: &mut client::MoodleClient, assignment_id: i64, pid: &Arc<AtomicU32>)
{
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("assignmentids[0]".to_string(), assignment_id.to_string());
    
    match client.post("mod_assign_get_submissions", &params).await {
        Ok(result) => {
            match result["assignments"][0]["submissions"].as_array() {
                Some(submissions) => {
                    for submission in submissions {
                        check_submission(client, submission, assignment_id, pid).await;
                    }
                },
                None => {
                    println!("No submissions found for assignment {}.", assignment_id);
                }
            }
            //println!("{:#?}", result["assignments"][0]["submissions"][0]);
        },
        Err(e) => {
            println!("Error fetching submissions: {e}");
        }
        }
}

async fn list_assignments(client: &mut client::MoodleClient)
{
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("courseids[0]".to_string(), COURSE_ID.to_string());

    match client.post("mod_assign_get_assignments", &params).await {
        Ok(result) => {
            match result["courses"][0]["assignments"].as_array() {
                Some(assignments) => {
                    for assignment in assignments {
                        let id = assignment["id"].as_i64().unwrap_or(-1);
                        let name = assignment["name"].as_str().unwrap_or("<null>");
                        println!("{id}: {name}");
                    }
                },
                None => {
                    println!("No assignments found in course {}.", COURSE_ID);
                }
            }
        },
        Err(e) => {
            println!("Error fetching assignments: {e}");
        }
    }
}

async fn list_courses(client: &mut client::MoodleClient)
{
    let mut params: HashMap<String, String> = HashMap::new();

    match client.post("core_course_get_courses", &params).await {
        Ok(result) => {
            match result.as_array() {
                Some(courses) => {
                    for course in courses {
                        println!("{:#?}", course);
                    }
                },
                None => {
                    println!("No courses found.");
                }
            }
        },
        Err(e) => {
            println!("Error fetching assignments: {e}");
        }
    }
}

#[tokio::main]
async fn main() {
    let pid = Arc::new(AtomicU32::new(0));
    let pid_clone = Arc::clone(&pid);
    ctrlc::set_handler(move || {
        let pid = pid_clone.load(Ordering::Relaxed);
        if pid > 0 {
            unsafe {
                GenerateConsoleCtrlEvent(1, pid);
            }        
        } else {
        }
    }).expect("Failed to set CTRL+C handler");    
    let file = fs::File::open("login.json").expect("Could not open JSON file with login information!");
    let reader = io::BufReader::new(file);
    let login_json:Value = serde_json::from_reader(reader).unwrap();
    let username = login_json["username"].as_str().unwrap();
    let password = login_json["password"].as_str().unwrap();
    match client::login(BASE_URL, username, password).await {
        Ok(token) => {
            let mut client = client::MoodleClient::new(BASE_URL, &token);
            //list_courses(&mut client).await;
            //list_assignments(&mut client).await;
            checker(&mut client, 2032, &pid).await;
        },
        Err(e) => {
            println!("Authentication failed: {}", e);
        }
    }
}

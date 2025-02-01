use reqwest::Client;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;
use std::io::{self, Write};

#[derive(Serialize, Clone)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    model: String,
    created_at: String,
    message: MessageResponse,
    done_reason: Option<String>,
    done: bool,
}

#[derive(Deserialize)]
struct MessageResponse {
    role: String,
    content: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    // Create and initialize conversation history with a system prompt.
    let mut conversation: Vec<Message> = vec![
        Message {
            role: "system".to_string(),
            content: "You are a helpful assistant.".to_string(),
        },
    ];

    println!("Welcome to the Rust Chatbot. Type your message below (or 'exit' to quit):");

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;
        let user_input = user_input.trim();

        if user_input.is_empty() {
            continue;
        }
        if user_input.eq_ignore_ascii_case("exit") || user_input.eq_ignore_ascii_case("quit") {
            println!("Goodbye!");
            break;
        }

        // Append the user's message to the conversation history.
        conversation.push(Message {
            role: "user".to_string(),
            content: user_input.to_string(),
        });

        // Create the request payload using the full conversation.
        let request_body = ChatRequest {
            model: "llama3.2".to_string(),
            messages: conversation.clone(), // Clone the history to send in the request.
        };

        let response = client
            .post("http://localhost:11434/api/chat")
            .json(&request_body)
            .send()
            .await?;

        // Process the streaming response.
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut assistant_message = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_str = std::str::from_utf8(&chunk)?;
            buffer.push_str(chunk_str);

            // Process complete lines in the buffer.
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].to_string();
                buffer.drain(..pos + 1);

                if !line.trim().is_empty() {
                    let chat_response: ChatResponse = serde_json::from_str(&line)?;
                    print!("{}", chat_response.message.content);
                    io::stdout().flush()?;
                    assistant_message.push_str(&chat_response.message.content);
                }
            }
        }
        println!(); // Newline after the assistant's reply.

        // Append the assistant's full reply to the conversation history.
        conversation.push(Message {
            role: "assistant".to_string(),
            content: assistant_message,
        });
    }

    Ok(())
}
//! Non-interactive fallback for ask_user prompts.
//!
//! Sequential numbered prompts for non-TTY environments.

use std::io::{self, Write};

use anyhow::Result;
use common::{Answer, AnswerType, AnswerValue, FormResponse, InteractionRequest};

/// Non-interactive fallback: sequential numbered prompts.
pub(super) fn prompt_non_interactive(request: &InteractionRequest) -> Result<FormResponse> {
    let mut answers = Vec::new();
    let num_questions = request.questions.len();

    for (idx, question) in request.questions.iter().enumerate() {
        println!(
            "\nQuestion {} of {}: {}",
            idx + 1,
            num_questions,
            question.text
        );

        let value = match &question.answer_type {
            AnswerType::SingleSelect { options } => {
                for (i, option) in options.iter().enumerate() {
                    println!("  {}) {}", i + 1, option.label);
                    if let Some(desc) = &option.description {
                        println!("     {}", desc);
                    }
                }

                print!("Choice [1]: ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let choice = input.trim();

                // On empty/EOF input, skip so enrichment can infer the value
                if choice.is_empty() {
                    AnswerValue::Skipped
                } else {
                    let selected_idx = choice.parse::<usize>().unwrap_or(1).saturating_sub(1);
                    if selected_idx < options.len() {
                        AnswerValue::Selected {
                            value: options[selected_idx].value.clone(),
                        }
                    } else {
                        AnswerValue::Skipped
                    }
                }
            }

            AnswerType::MultiSelect { options } => {
                for (i, option) in options.iter().enumerate() {
                    println!("  {}) {}", i + 1, option.label);
                    if let Some(desc) = &option.description {
                        println!("     {}", desc);
                    }
                }

                print!("Select (comma-separated) []: ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let choices: Vec<usize> = input
                    .trim()
                    .split(',')
                    .filter_map(|s| s.trim().parse::<usize>().ok())
                    .map(|n| n.saturating_sub(1))
                    .filter(|&i| i < options.len())
                    .collect();

                if choices.is_empty() {
                    AnswerValue::Skipped
                } else {
                    AnswerValue::MultiSelected {
                        values: choices.iter().map(|&i| options[i].value.clone()).collect(),
                    }
                }
            }

            AnswerType::YesNo { default } => {
                let default_str = if default.unwrap_or(false) {
                    "Y/n"
                } else {
                    "y/N"
                };
                print!("[{}]: ", default_str);
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let answer = match input.trim().to_lowercase().as_str() {
                    "y" | "yes" => true,
                    "n" | "no" => false,
                    "" => default.unwrap_or(false),
                    _ => default.unwrap_or(false),
                };

                AnswerValue::YesNo { answer }
            }

            AnswerType::FreeText { placeholder } => {
                if let Some(hint) = placeholder {
                    println!("  ({})", hint);
                }
                print!("> ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let text = input.trim().to_string();

                if text.is_empty() {
                    AnswerValue::Skipped
                } else {
                    AnswerValue::Text { content: text }
                }
            }
        };

        answers.push(Answer {
            question_id: question.id.clone(),
            value,
        });
    }

    // Print summary
    println!("\nAnswers:");
    for answer in &answers {
        let value_str = match &answer.value {
            AnswerValue::Selected { value } => value.clone(),
            AnswerValue::MultiSelected { values } => values.join(", "),
            AnswerValue::YesNo { answer } => if *answer { "Yes" } else { "No" }.to_string(),
            AnswerValue::Text { content } => format!("\"{}\"", content),
            AnswerValue::Skipped => "(skipped)".to_string(),
        };
        println!("  {}: {}", answer.question_id, value_str);
    }

    Ok(FormResponse::Completed(answers))
}

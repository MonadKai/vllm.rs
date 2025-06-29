use minijinja::{context, Environment};
#[cfg(feature = "python")]
use pyo3::pyclass;
use serde::Serialize;

#[cfg(feature = "python")]
#[pyclass]
#[derive(Serialize, Clone, Debug)]
pub struct Message {
    #[pyo3(get)]
    pub role: String,
    #[pyo3(get)]
    pub content: String,
}

#[cfg(not(feature = "python"))]
#[derive(Serialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[cfg(not(feature = "python"))]
impl Message {
    pub fn new(role: String, content: String) -> Self {
        Message { role, content }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ApplyChatTemplateError {
    #[error("failed to add chat template")]
    AddTemplateError(#[source] minijinja::Error),
    #[error("failed to get chat template")]
    GetTemplateError(#[source] minijinja::Error),
    #[error("failed to render chat template")]
    RenderTemplateError(#[source] minijinja::Error),
}

#[derive(Clone, Debug)]
pub struct ChatTemplate {
    system_message: Option<String>,
    chat_template: Option<String>,
    bos_token: Option<String>,
    eos_token: Option<String>,
    messages: Vec<Message>,
    add_generation_prompt: bool,
    enable_thinking: bool,
}

impl ChatTemplate {
    pub fn new(
        system_message: Option<String>,
        chat_template: Option<String>,
        bos_token: Option<String>,
        eos_token: Option<String>,
        prompt: Option<String>,
        add_generation_prompt: bool,
        enable_thinking: bool,
    ) -> Self {
        let mut template = ChatTemplate {
            system_message: system_message.clone(),
            chat_template,
            bos_token,
            eos_token,
            messages: Vec::new(),
            add_generation_prompt,
            enable_thinking,
        };
        if system_message.is_some() {
            template.append_message(
                "system".to_string(),
                template.system_message.clone().unwrap_or_default(),
            );
        }
        if let Some(prompt) = prompt {
            template.append_message("user".to_string(), prompt);
        }
        template
    }

    pub fn append_message(&mut self, role: String, content: String) {
        self.messages.push(Message { role, content });
    }

    pub fn set_messages(&mut self, messages: &Vec<Message>) {
        self.messages.clear();
        self.messages.extend(messages.clone());
    }

    #[allow(dead_code)]
    fn clear_message(&mut self) {
        self.messages.clear()
    }

    pub fn apply_chat_template(&self, log: bool) -> Result<String, ApplyChatTemplateError> {
        if self.chat_template.is_none() {
            return Err(ApplyChatTemplateError::GetTemplateError(
                minijinja::Error::new(minijinja::ErrorKind::CannotDeserialize, "Not found!"),
            ));
        }
        let mut env = Environment::new();
        env.set_lstrip_blocks(true);
        env.set_trim_blocks(true);
        env.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
        let template = self.chat_template.as_ref().unwrap();
        let mut template = template.replace("[::-1]", "|reverse");
        if template.find("{{ meta }}").is_some() {
            template = template.replace("{%- set meta = message.get(\"metadata\", \"\") %}", "");
            template = template.replace("{{ meta }}", "");
        }
        env.add_template("vllm.rs", template.as_str())
            .map_err(ApplyChatTemplateError::AddTemplateError)?;
        let template = env
            .get_template("vllm.rs")
            .map_err(ApplyChatTemplateError::GetTemplateError)?;

        if log {
            tracing::info!("messages {:?}", self.messages);
        }
        template
            .render(context! {
              messages => self.messages,
              add_generation_prompt => self.add_generation_prompt,
              bos_token => self.bos_token,
              eos_token => self.eos_token,
              enable_thinking => self.enable_thinking,
            })
            .map_err(ApplyChatTemplateError::RenderTemplateError)
    }
}

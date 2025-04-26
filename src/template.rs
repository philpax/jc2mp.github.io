use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use wikitext_simplified::{TemplateParameter, WikitextSimplifiedNode, parse_wiki_text_2};

use crate::page_context::PageContext;

pub struct Templates<'a> {
    pwt_configuration: &'a parse_wiki_text_2::Configuration,
    lookup: HashMap<String, PathBuf>,
    templates: HashMap<String, WikitextSimplifiedNode>,
}
impl<'a> Templates<'a> {
    pub fn new(
        src_root: &Path,
        pwt_configuration: &'a parse_wiki_text_2::Configuration,
    ) -> anyhow::Result<Self> {
        let mut lookup = HashMap::new();
        let templates = HashMap::new();

        fn scan_dir(
            src_root: &Path,
            path: &Path,
            lookup: &mut HashMap<String, PathBuf>,
        ) -> anyhow::Result<()> {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    scan_dir(src_root, &path, lookup)?;
                } else if path.is_file() {
                    let key = path
                        .strip_prefix(src_root)?
                        .with_extension("")
                        .as_os_str()
                        .to_string_lossy()
                        .to_lowercase()
                        .replace("\\", "/")
                        .replace(" ", "_");
                    lookup.insert(key, path);
                }
            }

            Ok(())
        }

        scan_dir(src_root, src_root, &mut lookup)?;

        Ok(Self {
            pwt_configuration,
            lookup,
            templates,
        })
    }

    fn get(&mut self, name: &str) -> anyhow::Result<&WikitextSimplifiedNode> {
        let key = name.to_lowercase().replace(" ", "_");
        let path = self
            .lookup
            .get(&key)
            .ok_or(anyhow::anyhow!("Template not found: {name} -> {key}"))?;
        let content = fs::read_to_string(path)?;
        let simplified =
            wikitext_simplified::parse_and_simplify_wikitext(&content, self.pwt_configuration)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to parse and simplify wiki file {}: {e:?}",
                        path.display()
                    )
                })?;
        self.templates.insert(
            key.to_string(),
            WikitextSimplifiedNode::Fragment {
                children: simplified,
            },
        );

        Ok(&self.templates[&key])
    }

    /// Instantiate the template by replacing all template parameter uses with their values,
    /// instantiating nested templates, converting back to wikitext, and then doing this until
    /// no more template parameter uses or nested templates are found.
    ///
    /// God, I love wikitext.
    pub fn instantiate(
        &mut self,
        pwt_configuration: &parse_wiki_text_2::Configuration,
        template: TemplateToInstantiate,
        parameters: &[TemplateParameter],
        page_context: &PageContext,
    ) -> WikitextSimplifiedNode {
        use WikitextSimplifiedNode as WSN;

        let mut template = match template {
            TemplateToInstantiate::Name(name) => {
                if name.eq_ignore_ascii_case("subpagename") {
                    return WSN::Text {
                        text: page_context.sub_page_name.to_string(),
                    };
                }
                self.get(name).unwrap().clone()
            }
            TemplateToInstantiate::Node(node) => node,
        };

        // Check if we're done
        let mut further_instantiation_required = false;
        template.visit(&mut |node| {
            further_instantiation_required |= matches!(
                node,
                WSN::TemplateParameterUse { .. } | WSN::Template { .. }
            );
        });
        if !further_instantiation_required {
            return template;
        }

        // Instantiate all nested templates, and replace
        template.visit_and_replace_mut(&mut |node| match node {
            WSN::Template { name, parameters } => self.instantiate(
                pwt_configuration,
                TemplateToInstantiate::Name(name),
                parameters,
                page_context,
            ),
            WSN::TemplateParameterUse { name, default } => {
                let parameter = parameters
                    .iter()
                    .find(|p| p.name == *name)
                    .map(|p| p.value.clone())
                    .or_else(|| {
                        name.eq_ignore_ascii_case("subpagename")
                            .then(|| page_context.sub_page_name.to_string())
                    });
                if let Some(parameter) = parameter {
                    WSN::Text { text: parameter }
                } else if let Some(default) = default {
                    WSN::Text {
                        text: WSN::Fragment {
                            children: default.clone(),
                        }
                        .to_wikitext(),
                    }
                } else {
                    WSN::Text {
                        text: "".to_string(),
                    }
                }
            }
            _ => node.clone(),
        });

        // Convert the template back to wikitext, then reparse it, and then send it through again
        let template_wikitext = template.to_wikitext();
        let roundtripped_template =
            wikitext_simplified::parse_and_simplify_wikitext(&template_wikitext, pwt_configuration)
                .unwrap_or_else(|e| {
                    panic!("Failed to parse and simplify template {template_wikitext}: {e:?}")
                });
        self.instantiate(
            pwt_configuration,
            TemplateToInstantiate::Node(WikitextSimplifiedNode::Fragment {
                children: roundtripped_template,
            }),
            parameters,
            page_context,
        )
    }
}

#[derive(Clone, Debug)]
pub enum TemplateToInstantiate<'a> {
    Name(&'a str),
    Node(WikitextSimplifiedNode),
}

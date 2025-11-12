use std::{fs, path::Path, sync::OnceLock};

use template::{TemplateToInstantiate, Templates};
use wikitext_simplified::{WikitextSimplifiedNode, wikitext_util::parse_wiki_text_2};

mod page_context;
use page_context::PageContext;

mod syntax;
mod template;

const WIKI_DIRECTORY: &str = "wiki";

static SYNTAX_HIGHLIGHTER: OnceLock<syntax::SyntaxHighlighter> = OnceLock::new();

fn main() -> anyhow::Result<()> {
    let output_dir = Path::new("output");
    let _ = fs::remove_dir_all(output_dir);
    fs::create_dir_all(output_dir)?;

    // Copy the contents of the `static` folder into the output directory
    copy_files_recursively(Path::new("static"), output_dir)?;

    // Generate wiki
    generate_wiki(Path::new(WIKI_DIRECTORY), &output_dir.join(WIKI_DIRECTORY))?;

    Ok(())
}

fn copy_files_recursively(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let new_path = dst.join(path.file_name().unwrap());

        if path.is_dir() {
            fs::create_dir_all(&new_path)?;
            copy_files_recursively(&path, &new_path)?;
        } else {
            fs::copy(&path, &new_path)?;
        }
    }

    Ok(())
}

fn generate_wiki(src: &Path, dst: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;

    let pwt_configuration = wikitext_simplified::wikitext_util::wikipedia_pwt_configuration();
    let loader = template::FileSystemLoader::new(src)?;
    let mut templates = Templates::new(loader, &pwt_configuration)?;

    // Initialize syntax highlighter
    let highlighter = SYNTAX_HIGHLIGHTER.get_or_init(syntax::SyntaxHighlighter::default);

    // Generate syntax highlighting CSS
    let syntax_css = highlighter.theme_css();
    let output_dir = dst.parent().unwrap();
    fs::create_dir_all(output_dir.join("style"))?;
    fs::write(output_dir.join("style/syntax.css"), syntax_css)?;

    generate_wiki_folder(&mut templates, src, dst, dst, &pwt_configuration)?;
    redirect(&page_title_to_route_path("Main_Page").url_path())
        .write_to_route(dst, paxhtml::RoutePath::new([], "index.html".to_string()))?;

    Ok(())
}

fn generate_wiki_folder(
    templates: &mut Templates,
    src: &Path,
    dst_root: &Path,
    dst: &Path,
    pwt_configuration: &parse_wiki_text_2::Configuration,
) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;

    let files = fs::read_dir(src)?;
    for file in files {
        let file = file?;
        let path = file.path();

        if path.is_dir() {
            generate_wiki_folder(
                templates,
                &path,
                dst_root,
                &dst.join(path.file_name().unwrap()),
                pwt_configuration,
            )?;
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let simplified =
            wikitext_simplified::parse_and_simplify_wikitext(&content, pwt_configuration).map_err(
                |e| {
                    anyhow::anyhow!(
                        "Failed to parse and simplify wiki file {}: {e:?}",
                        path.display()
                    )
                },
            )?;

        let output_json = dst.join(path.with_extension("json").file_name().unwrap());
        fs::write(&output_json, serde_json::to_string_pretty(&simplified)?)?;

        let output_html = output_json.with_extension("html");
        let output_html_rel = output_html.strip_prefix(dst_root).unwrap();

        let route_path = paxhtml::RoutePath::new(
            output_html_rel.parent().iter().flat_map(|p| {
                p.components().filter_map(|comp| match comp {
                    std::path::Component::Normal(name) => name.to_str(),
                    _ => None,
                })
            }),
            output_html_rel
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string()),
        );

        let document = if let [WikitextSimplifiedNode::Redirect { target }] = simplified.as_slice()
        {
            redirect(&page_title_to_route_path(target).url_path())
        } else {
            let sub_page_name = path
                .with_extension("")
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();

            let page_context = PageContext {
                input_path: path,
                title: output_html_rel
                    .with_extension("")
                    .to_str()
                    .map(|s| s.to_string())
                    .unwrap()
                    .replace("\\", "/")
                    .replace("_", " "),
                route_path: route_path.clone(),
                sub_page_name,
            };

            layout(
                &page_context.title,
                paxhtml::Element::from_iter(simplified.iter().map(|node| {
                    convert_wikitext_to_html(templates, pwt_configuration, node, &page_context)
                })),
            )
        };

        document.write_to_route(dst_root, route_path)?;
    }

    Ok(())
}

fn layout(title: &str, inner: paxhtml::Element) -> paxhtml::Document {
    let mut links = vec![(
        "Home",
        paxhtml::RoutePath::new(
            std::iter::once(WIKI_DIRECTORY),
            Some("Main_Page.html".to_string()),
        ),
    )];

    if title != "Main Page" {
        let mut components = vec![];
        for component in title.split('/') {
            let route_path = paxhtml::RoutePath::new(
                std::iter::once(WIKI_DIRECTORY).chain(components.iter().copied()),
                Some(format!("{}.html", component.replace(" ", "_"))),
            );
            links.push((component, route_path));
            components.push(component);
        }
    }

    let mut breadcrumbs = vec![];
    for (idx, (component, route_path)) in links.into_iter().enumerate() {
        if idx > 0 {
            breadcrumbs.push(paxhtml::html! { " / " });
        }
        breadcrumbs.push(paxhtml::html! { <a href={route_path.url_path()}>{component}</a> });
    }

    paxhtml::Document::new([
        paxhtml::builder::doctype(["html".into()]),
        paxhtml::html! {
            <html lang="en">
            <head>
                <meta charset="UTF-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                <title>{format!("JC2-MP Documentation - {title}")}</title>
                <link href="/style/bootstrap.min.css" rel="stylesheet" />
                <link href="/style/syntax.css" rel="stylesheet" />
                <style>
                    "body { background-color: #f8f9fa; }
                    .container { max-width: 1200px; }
                    .content-wrapper { background-color: white; padding: 2rem; border-radius: 0.5rem; box-shadow: 0 0.125rem 0.25rem rgba(0,0,0,0.075); }
                    pre { background-color: #2b303b; padding: 1rem; border-radius: 0.375rem; overflow-x: auto; }
                    pre code { color: #c0c5ce; }
                    h1 { border-bottom: 2px solid #dee2e6; padding-bottom: 0.5rem; margin-bottom: 1.5rem; }
                    h2 { margin-top: 2rem; margin-bottom: 1rem; }
                    h3 { margin-top: 1.5rem; margin-bottom: 0.75rem; }
                    .navbar-brand { font-weight: 600; }"
                </style>
            </head>
            <body>
                <nav class="navbar navbar-expand-lg navbar-dark bg-dark mb-4">
                    <div class="container">
                        <a class="navbar-brand" href="/wiki">"Just Cause 2: Multiplayer"</a>
                        <button class="navbar-toggler" r#type="button" dataBsToggle="collapse" dataBsTarget="#navbarNav" ariaControls="navbarNav" ariaExpanded="false" ariaLabel="Toggle navigation">
                            <span class="navbar-toggler-icon"></span>
                        </button>
                        <div class="collapse navbar-collapse" id="navbarNav">
                            <ul class="navbar-nav ms-auto">
                                <li class="nav-item">
                                    <a class="nav-link" href="/">"Website"</a>
                                </li>
                            </ul>
                        </div>
                    </div>
                </nav>
                <div class="container">
                    <div class="content-wrapper">
                        <h1>#{breadcrumbs}</h1>
                        {inner}
                    </div>
                </div>
                <script src="/js/bootstrap.bundle.min.js"></script>
            </body>
            </html>
        },
    ])
}

fn convert_wikitext_to_html(
    templates: &mut Templates,
    pwt_configuration: &parse_wiki_text_2::Configuration,
    node: &WikitextSimplifiedNode,
    page_context: &PageContext,
) -> paxhtml::Element {
    use WikitextSimplifiedNode as WSN;
    use paxhtml::html;

    fn parse_attributes_from_wsn(
        templates: &mut Templates,
        pwt_configuration: &parse_wiki_text_2::Configuration,
        page_context: &PageContext,
        attributes_context: &str,
        attributes: &[WSN],
    ) -> Vec<paxhtml::Attribute> {
        if attributes.is_empty() {
            return vec![];
        }
        // Instantiate the attributes before extracting the text
        let attributes = templates.instantiate(
            pwt_configuration,
            TemplateToInstantiate::Node(WikitextSimplifiedNode::Fragment {
                children: attributes.to_vec(),
            }),
            &[],
            page_context,
        );
        let WSN::Fragment {
            children: attributes,
        } = attributes
        else {
            panic!(
                "Table {attributes_context} attributes was not a fragment after instantiation; got {attributes:?} in {page_context}"
            );
        };

        // Merge all text nodes into a single string
        let merged_text = attributes
            .iter()
            .filter_map(|node| {
                if let WSN::Text { text } = node {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        if merged_text.is_empty() && !attributes.is_empty() {
            panic!(
                "Table {attributes_context} attributes must contain text; got {attributes:?} in {page_context}"
            );
        }

        paxhtml::Attribute::parse_from_str(&merged_text).unwrap()
    }

    fn parse_optional_attributes_from_wsn(
        templates: &mut Templates,
        pwt_configuration: &parse_wiki_text_2::Configuration,
        page_context: &PageContext,
        attributes_context: &str,
        attributes: &Option<Vec<WSN>>,
    ) -> Vec<paxhtml::Attribute> {
        attributes
            .as_deref()
            .map(|attributes| {
                parse_attributes_from_wsn(
                    templates,
                    pwt_configuration,
                    page_context,
                    attributes_context,
                    attributes,
                )
            })
            .unwrap_or_default()
    }

    let convert_children = |templates: &mut Templates, children: &[WikitextSimplifiedNode]| {
        paxhtml::Element::from_iter(
            children
                .iter()
                .skip_while(|node| matches!(node, WSN::ParagraphBreak | WSN::Newline))
                .map(|node| {
                    convert_wikitext_to_html(templates, pwt_configuration, node, page_context)
                }),
        )
    };

    match node {
        WSN::Fragment { children } => convert_children(templates, children),
        WSN::Template { name, parameters } => {
            let template = templates.instantiate(
                pwt_configuration,
                TemplateToInstantiate::Name(name),
                parameters,
                page_context,
            );
            convert_wikitext_to_html(templates, pwt_configuration, &template, page_context)
        }
        tpu @ WSN::TemplateParameterUse { .. } => {
            html! { <>{tpu.to_wikitext()}</> }
        }
        WSN::Heading { level, children } => {
            paxhtml::builder::tag(format!("h{level}"), None, false)(convert_children(
                templates, children,
            ))
        }
        WSN::Link { text, title } => {
            html! {
                <a href={page_title_to_route_path(title).url_path()}>
                    {paxhtml::Element::Raw { html: text.to_string() }}
                </a>
            }
        }
        WSN::ExtLink { link, text } => {
            html! {
                <a href={link}>
                    {paxhtml::Element::Raw { html: text.as_ref().unwrap_or(link).to_string() }}
                </a>
            }
        }
        WSN::Bold { children } => {
            html! { <strong>{convert_children(templates, children)}</strong> }
        }
        WSN::Italic { children } => {
            html! { <em>{convert_children(templates, children)}</em> }
        }
        WSN::Blockquote { children } => {
            html! { <blockquote>{convert_children(templates, children)}</blockquote> }
        }
        WSN::Superscript { children } => {
            html! { <sup>{convert_children(templates, children)}</sup> }
        }
        WSN::Subscript { children } => {
            html! { <sub>{convert_children(templates, children)}</sub> }
        }
        WSN::Small { children } => {
            html! { <small>{convert_children(templates, children)}</small> }
        }
        WSN::Preformatted { children } => {
            html! { <pre>{convert_children(templates, children)}</pre> }
        }
        WSN::Tag {
            name,
            attributes,
            children,
        } => {
            if name == "syntaxhighlight" {
                // Extract language from attributes string before parsing, defaulting to Lua
                let attrs_str = attributes.as_deref().unwrap_or_default();
                let lang = if attrs_str.contains("lang=") || attrs_str.contains("language=") {
                    // Simple extraction of lang attribute value
                    attrs_str.split_whitespace().find_map(|part| {
                        if let Some(value) = part.strip_prefix("lang=") {
                            Some(value.trim_matches('"').trim_matches('\''))
                        } else if let Some(value) = part.strip_prefix("language=") {
                            Some(value.trim_matches('"').trim_matches('\''))
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };

                // Get the code text
                let code = if let [WSN::Text { text }] = children.as_slice() {
                    text.trim()
                } else {
                    // If not simple text, fall back to plain rendering
                    let parsed_attributes = paxhtml::Attribute::parse_from_str(attrs_str).unwrap();
                    return html! { <pre {parsed_attributes}><code>{convert_children(templates, children)}</code></pre> };
                };

                // Use syntax highlighter
                if let Some(highlighter) = SYNTAX_HIGHLIGHTER.get() {
                    match highlighter.highlight_code(lang, code) {
                        Ok(highlighted) => {
                            html! { <pre><code>{highlighted}</code></pre> }
                        }
                        Err(_) => {
                            // Fallback to plain text if highlighting fails
                            let parsed_attributes =
                                paxhtml::Attribute::parse_from_str(attrs_str).unwrap();
                            html! { <pre {parsed_attributes}><code>{code}</code></pre> }
                        }
                    }
                } else {
                    // Fallback if highlighter not initialized
                    let parsed_attributes = paxhtml::Attribute::parse_from_str(attrs_str).unwrap();
                    html! { <pre {parsed_attributes}><code>{code}</code></pre> }
                }
            } else {
                let parsed_attributes =
                    paxhtml::Attribute::parse_from_str(attributes.as_deref().unwrap_or_default())
                        .unwrap();
                let children = convert_children(templates, children);
                paxhtml::builder::tag(name.to_string(), parsed_attributes, false)(children)
            }
        }
        WSN::Text { text } => paxhtml::Element::Raw {
            html: text.to_string(),
        },
        WSN::Table {
            attributes,
            captions,
            rows,
        } => {
            // Add Bootstrap classes to table attributes
            let mut modified_attributes = attributes.clone();

            // Add Bootstrap table classes if not already present
            let has_class_attr = if !attributes.is_empty() {
                // Check if there's already a class attribute by instantiating and checking text
                let instantiated = templates.instantiate(
                    pwt_configuration,
                    TemplateToInstantiate::Node(WikitextSimplifiedNode::Fragment {
                        children: attributes.to_vec(),
                    }),
                    &[],
                    page_context,
                );
                if let WSN::Fragment { children } = instantiated {
                    if let Some(WSN::Text { text }) = children.first() {
                        text.contains("class=")
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !has_class_attr {
                // Add Bootstrap table classes
                modified_attributes.push(WSN::Text {
                    text: " class=\"table table-striped table-bordered table-hover\"".to_string(),
                });
            }

            let attributes = parse_attributes_from_wsn(
                templates,
                pwt_configuration,
                page_context,
                "main",
                &modified_attributes,
            );
            html! {
                <table {attributes}>
                    <thead class="table-dark">
                        <tr>
                            #{captions
                                .iter()
                                .map(|caption| {
                                    let attributes = parse_optional_attributes_from_wsn(
                                        templates,
                                        pwt_configuration,
                                        page_context,
                                        "caption",
                                        &caption.attributes,
                                    );
                                    html! {
                                        <th {attributes}>
                                            {convert_children(templates, &caption.content)}
                                        </th>
                                    }
                                })
                            }
                        </tr>
                    </thead>
                    <tbody>
                        #{rows
                            .iter()
                            .map(|row| {
                                let attributes = parse_attributes_from_wsn(
                                    templates,
                                    pwt_configuration,
                                    page_context,
                                    "row",
                                    &row.attributes,
                                );
                                html! {
                                    <tr {attributes}>
                                        #{row.cells
                                            .iter()
                                            .map(|cell| {
                                                let attributes = parse_optional_attributes_from_wsn(
                                                    templates,
                                                    pwt_configuration,
                                                    page_context,
                                                    "cell",
                                                    &cell.attributes,
                                                );
                                                html! {
                                                    <td {attributes}>
                                                        {convert_children(templates, &cell.content)}
                                                    </td>
                                                }
                                            })
                                        }
                                    </tr>
                                }
                            })
                        }
                    </tbody>
                </table>
            }
        }
        WSN::OrderedList { items } => {
            html! {
                <ol>
                    #{items
                        .iter()
                        .map(|i| {
                            html! { <li>{convert_children(templates, &i.content)}</li> }
                        })
                    }
                </ol>
            }
        }
        WSN::UnorderedList { items } => {
            html! {
                <ul>
                    #{items
                        .iter()
                        .map(|i| {
                            html! { <li>{convert_children(templates, &i.content)}</li> }
                        })
                    }
                </ul>
            }
        }
        WSN::DefinitionList { items } => {
            use wikitext_simplified::DefinitionListItemType;
            html! {
                <dl>
                    #{items.iter().map(|i| {
                        let children = convert_children(templates, &i.content);
                        match i.type_ {
                            DefinitionListItemType::Term => html! { <dt>{children}</dt> },
                            DefinitionListItemType::Details => html! { <dd>{children}</dd> },
                        }
                    })}
                </dl>
            }
        }
        WSN::Redirect { target } => html! {
            <a href={page_title_to_route_path(target).url_path()}>
                "REDIRECT: "{target}
            </a>
        },
        WSN::HorizontalDivider => html! { <hr /> },
        WSN::ParagraphBreak => html! { <br /> },
        WSN::Newline => html! { <br /> },
    }
}

fn page_title_to_route_path(title: &str) -> paxhtml::RoutePath {
    let title_link = title.replace(" ", "_");
    let segments = title_link.split('/').collect::<Vec<_>>();
    let (page_name, directories) = segments.split_last().unwrap();

    paxhtml::RoutePath::new(
        std::iter::once(WIKI_DIRECTORY).chain(directories.iter().copied()),
        Some(format!("{page_name}.html")),
    )
}

fn redirect(to_url: &str) -> paxhtml::Document {
    paxhtml::Document::new([
        paxhtml::builder::doctype(["html".into()]),
        paxhtml::html! {
            <html>
                <head>
                    <title>"Redirecting..."</title>
                    <meta charset="utf-8" />
                    <meta httpEquiv="refresh" content={format!("0; url={to_url}")} />
                </head>
                <body>
                    <p>"Redirecting..."</p>
                    <p>
                        <a href={to_url} title="Click here if you are not redirected">
                            "Click here"
                        </a>
                    </p>
                </body>
            </html>
        },
    ])
}

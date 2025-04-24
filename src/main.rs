use std::{fs, path::Path};

use template::{TemplateToInstantiate, Templates};
use wikitext_simplified::{WikitextSimplifiedNode, wikitext_util::parse_wiki_text_2};

mod template;

const WIKI_DIRECTORY: &str = "wiki";

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
    let mut templates = Templates::new(src, &pwt_configuration)?;

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
        } else {
            let content = fs::read_to_string(&path)?;
            let simplified =
                wikitext_simplified::parse_and_simplify_wikitext(&content, pwt_configuration)
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to parse and simplify wiki file {}: {e:?}",
                            path.display()
                        )
                    })?;

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

            let sub_page_name = path
                .with_extension("")
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();

            let title = output_html_rel
                .with_extension("")
                .to_str()
                .map(|s| s.to_string())
                .unwrap()
                .replace("\\", "/")
                .replace("_", " ");

            let document = if let [WikitextSimplifiedNode::Redirect { target }] =
                simplified.as_slice()
            {
                redirect(&page_title_to_route_path(target).url_path())
            } else {
                layout(
                    &title,
                    paxhtml::Element::from_iter(simplified.iter().map(|node| {
                        convert_wikitext_to_html(templates, pwt_configuration, node, &sub_page_name)
                    })),
                )
            };

            document.write_to_route(dst_root, route_path)?;
        }
    }

    Ok(())
}

fn layout(title: &str, inner: paxhtml::Element) -> paxhtml::Document {
    paxhtml::Document::new([
        paxhtml::builder::doctype(["html".into()]),
        paxhtml::html! {
            <html lang="en">
            <head>
                <meta charset="UTF-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                <title>{format!("JC2-MP Documentation - {title}")}</title>
                <link href="/style/bootstrap.min.css" rel="stylesheet" />
            </head>
            <body>
                <nav class="navbar navbar-expand-lg navbar-dark bg-dark">
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
                <div class="container mt-4">
                    <h1>{title}</h1>
                    {inner}
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
    sub_page_name: &str,
) -> paxhtml::Element {
    use WikitextSimplifiedNode as WSN;
    use paxhtml::html;

    fn parse_optional_attributes(attributes: &Option<String>) -> Vec<paxhtml::Attribute> {
        paxhtml::Attribute::parse_from_str(attributes.as_deref().unwrap_or_default()).unwrap()
    }

    let mut convert_children = |children: &[WikitextSimplifiedNode]| {
        paxhtml::Element::from_iter(children.iter().map(|node| {
            convert_wikitext_to_html(templates, pwt_configuration, node, sub_page_name)
        }))
    };

    match node {
        WSN::Fragment { children } => convert_children(children),
        WSN::Template { name, parameters } => {
            let template = templates.instantiate(
                pwt_configuration,
                TemplateToInstantiate::Name(name),
                parameters,
                sub_page_name,
            );
            // if sub_page_name == "GetCellId" {
            //     dbg!(&template);
            // }
            convert_wikitext_to_html(templates, pwt_configuration, &template, sub_page_name)
        }
        tpu @ WSN::TemplateParameterUse { .. } => {
            html! { <>{tpu.to_wikitext()}</> }
        }
        WSN::Heading { level, children } => {
            paxhtml::builder::tag(format!("h{level}"), None, false)(convert_children(children))
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
            html! { <strong>{convert_children(children)}</strong> }
        }
        WSN::Italic { children } => {
            html! { <em>{convert_children(children)}</em> }
        }
        WSN::Blockquote { children } => {
            html! { <blockquote>{convert_children(children)}</blockquote> }
        }
        WSN::Superscript { children } => {
            html! { <sup>{convert_children(children)}</sup> }
        }
        WSN::Subscript { children } => {
            html! { <sub>{convert_children(children)}</sub> }
        }
        WSN::Small { children } => {
            html! { <small>{convert_children(children)}</small> }
        }
        WSN::Preformatted { children } => {
            html! { <pre>{convert_children(children)}</pre> }
        }
        WSN::Tag {
            name,
            attributes,
            children,
        } => {
            let attributes = parse_optional_attributes(attributes);
            if name == "syntaxhighlight" {
                if let [WSN::Text { text }] = children.as_slice() {
                    html! { <pre {attributes}><code>{text.trim()}</code></pre> }
                } else {
                    html! { <pre {attributes}><code>{convert_children(children)}</code></pre> }
                }
            } else {
                let children = convert_children(children);
                paxhtml::builder::tag(name.to_string(), attributes, false)(children)
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
            let attributes = paxhtml::Attribute::parse_from_str(attributes).unwrap();
            html! {
                <table {attributes}>
                    <thead>
                        <tr>
                            #{captions
                                .iter()
                                .map(|caption| {
                                    html! {
                                        <th {parse_optional_attributes(&caption.attributes)}>
                                            {convert_children(&caption.content)}
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
                                html! {
                                    <tr {parse_optional_attributes(&row.attributes)}>
                                        #{row.cells
                                            .iter()
                                            .map(|cell| {
                                                html! {
                                                    <td {parse_optional_attributes(&cell.attributes)}>
                                                        {convert_children(&cell.content)}
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
                            html! { <li>{convert_children(&i.content)}</li> }
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
                            html! { <li>{convert_children(&i.content)}</li> }
                        })
                    }
                </ul>
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

use std::{fs, path::Path};

use wikitext_simplified::WikitextSimplifiedNode;

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

    generate_wiki_folder(src, dst, dst, &pwt_configuration)?;
    redirect(&page_title_to_route_path("Main_Page").url_path())
        .write_to_route(dst, paxhtml::RoutePath::new([], "index.html".to_string()))?;

    Ok(())
}

fn generate_wiki_folder(
    src: &Path,
    dst_root: &Path,
    dst: &Path,
    pwt_configuration: &wikitext_simplified::wikitext_util::parse_wiki_text_2::Configuration,
) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;

    let files = fs::read_dir(src)?;
    for file in files {
        let file = file?;
        let path = file.path();

        if path.is_dir() {
            generate_wiki_folder(
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
                output_html_rel
                    .parent()
                    .iter()
                    .map(|p| {
                        p.components().filter_map(|comp| match comp {
                            std::path::Component::Normal(name) => name.to_str(),
                            _ => None,
                        })
                    })
                    .flatten(),
                output_html_rel
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string()),
            );

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
                    paxhtml::Element::from_iter(simplified.iter().map(convert_wikitext_to_html)),
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

fn convert_wikitext_to_html(node: &WikitextSimplifiedNode) -> paxhtml::Element {
    use WikitextSimplifiedNode as WSN;
    use paxhtml::html;

    fn parse_optional_attributes(attributes: &Option<String>) -> Vec<paxhtml::Attribute> {
        paxhtml::Attribute::parse_from_str(attributes.as_deref().unwrap_or_default()).unwrap()
    }

    match node {
        WSN::Fragment { children } => {
            paxhtml::Element::from_iter(children.iter().map(convert_wikitext_to_html))
        }
        WSN::Template { name, children } => html! { <>"template " {name}</> },
        WSN::TemplateParameterUse { name, default } => {
            html! { <>"template parameter use " {name}</> }
        }
        WSN::Heading { level, children } => {
            paxhtml::builder::tag(format!("h{level}"), None, false)(paxhtml::Element::from_iter(
                children.iter().map(convert_wikitext_to_html),
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
                    {paxhtml::Element::Raw { html: text.as_ref().unwrap_or(&link).to_string() }}
                </a>
            }
        }
        WSN::Bold { children } => {
            html! { <strong>#{children.iter().map(convert_wikitext_to_html)}</strong> }
        }
        WSN::Italic { children } => {
            html! { <em>#{children.iter().map(convert_wikitext_to_html)}</em> }
        }
        WSN::Blockquote { children } => {
            html! { <blockquote>#{children.iter().map(convert_wikitext_to_html)}</blockquote> }
        }
        WSN::Superscript { children } => {
            html! { <sup>#{children.iter().map(convert_wikitext_to_html)}</sup> }
        }
        WSN::Subscript { children } => {
            html! { <sub>#{children.iter().map(convert_wikitext_to_html)}</sub> }
        }
        WSN::Small { children } => {
            html! { <small>#{children.iter().map(convert_wikitext_to_html)}</small> }
        }
        WSN::Preformatted { children } => {
            html! { <pre>#{children.iter().map(convert_wikitext_to_html)}</pre> }
        }
        WSN::Tag {
            name,
            attributes,
            children,
        } => paxhtml::builder::tag(
            name.to_string(),
            parse_optional_attributes(attributes),
            false,
        )(paxhtml::Element::from_iter(
            children.iter().map(convert_wikitext_to_html),
        )),
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
                                            #{caption.content.iter().map(convert_wikitext_to_html)}
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
                                                        #{cell.content.iter().map(convert_wikitext_to_html)}
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
                            html! { <li>#{i.content.iter().map(convert_wikitext_to_html)}</li> }
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
                            html! { <li>#{i.content.iter().map(convert_wikitext_to_html)}</li> }
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

use std::{fs, path::Path};

fn main() -> anyhow::Result<()> {
    let output_dir = Path::new("output");
    let _ = fs::remove_dir_all(output_dir);
    fs::create_dir_all(output_dir)?;

    // Copy the contents of the `static` folder into the output directory
    copy_files_recursively(Path::new("static"), output_dir)?;

    // Generate wiki
    generate_wiki(Path::new("wiki"), &output_dir.join("wiki"))?;

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

            layout(paxhtml::Element::from_iter(
                simplified.iter().map(convert_wikitext_to_html),
            ))
            .write_to_route(dst_root, route_path)?;
        }
    }

    Ok(())
}

fn layout(inner: paxhtml::Element) -> paxhtml::Document {
    paxhtml::Document::new([
        paxhtml::builder::doctype(["html".into()]),
        paxhtml::html! {
            <html lang="en">
            <head>
                <meta charset="UTF-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                <title>"JC2-MP Documentation"</title>
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
                    {inner}
                </div>
                <script src="/js/bootstrap.bundle.min.js"></script>
            </body>
            </html>
        },
    ])
}

fn convert_wikitext_to_html(
    node: &wikitext_simplified::WikitextSimplifiedNode,
) -> paxhtml::Element {
    use paxhtml::html;
    use wikitext_simplified::WikitextSimplifiedNode as WSN;

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
            html! { <a href={title}>{text}</a> }
        }
        WSN::ExtLink { link, text } => {
            html! { <a href={link}>{text.as_ref().unwrap_or(&link)}</a> }
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
            paxhtml::Attribute::parse_from_str(attributes.as_deref().unwrap_or_default()).unwrap(),
            false,
        )(paxhtml::Element::from_iter(
            children.iter().map(convert_wikitext_to_html),
        )),
        WSN::Text { text } => html! { {text} },
        WSN::Table {
            attributes,
            captions,
            rows,
        } => html! { "table" },
        WSN::OrderedList { items } => {
            html! {
                <ol>
                    {items
                        .iter()
                        .map(|i| {
                            html! { <li>#{i.content.iter().map(convert_wikitext_to_html)}</li> }
                        })
                        .collect::<Vec<_>>()
                    }
                </ol>
            }
        }
        WSN::UnorderedList { items } => {
            html! {
                <ul>
                    {items
                        .iter()
                        .map(|i| {
                            html! { <li>#{i.content.iter().map(convert_wikitext_to_html)}</li> }
                        })
                        .collect::<Vec<_>>()
                    }
                </ul>
            }
        }
        WSN::Redirect { target } => html! { <a href={target}>{target}</a> },
        WSN::HorizontalDivider => html! { <hr /> },
        WSN::ParagraphBreak => html! { <br /> },
        WSN::Newline => html! { <br /> },
    }
}

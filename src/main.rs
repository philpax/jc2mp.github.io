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

            // Get parent folders of output_html_rel as a vec of strings
            let parent_folders: Vec<String> = output_html_rel
                .parent()
                .map(|p| {
                    p.components()
                        .filter_map(|comp| match comp {
                            std::path::Component::Normal(name) => {
                                Some(name.to_string_lossy().into_owned())
                            }
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();

            layout(paxhtml::Element::Empty).write_to_route(
                dst_root,
                paxhtml::RoutePath::new(
                    parent_folders.iter().map(|s| s.as_str()),
                    output_html_rel
                        .file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string()),
                ),
            )?;
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
                        <a class="navbar-brand" href="/">"Just Cause 2: Multiplayer"</a>
                        <button class="navbar-toggler" r#type="button" dataBsToggle="collapse" dataBsTarget="#navbarNav" ariaControls="navbarNav" ariaExpanded="false" ariaLabel="Toggle navigation">
                            <span class="navbar-toggler-icon"></span>
                        </button>
                        <div class="collapse navbar-collapse" id="navbarNav">
                            <ul class="navbar-nav ms-auto">
                                <li class="nav-item">
                                    <a class="nav-link" href="/wiki">"Wiki"</a>
                                </li>
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

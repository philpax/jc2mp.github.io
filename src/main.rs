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

    generate_wiki_folder(src, dst, &pwt_configuration)?;

    Ok(())
}

fn generate_wiki_folder(
    src: &Path,
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

            let output_path = dst.join(path.with_extension("json").file_name().unwrap());
            fs::write(output_path, serde_json::to_string_pretty(&simplified)?)?;
        }
    }

    Ok(())
}

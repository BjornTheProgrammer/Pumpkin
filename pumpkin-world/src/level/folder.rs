use std::path::PathBuf;

#[derive(Clone)]
pub struct LevelFolder {
    pub root_folder: PathBuf,
    pub region_folder: PathBuf,
    pub entities_folder: PathBuf,
}

impl LevelFolder {
    pub fn new(root_folder: PathBuf) -> Self {
        let region_folder = root_folder.join("region");
        let entities_folder = root_folder.join("entities");

        Self {
            root_folder,
            region_folder,
            entities_folder,
        }
    }
}

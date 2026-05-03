use coding_agents_md::walk_agents_md;
use proptest::prelude::*;
use std::fs;
use tempfile::TempDir;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn k14_walk_is_deterministic(
        depths in prop::collection::vec(1usize..5, 1..6),
        with_md in prop::collection::vec(any::<bool>(), 6),
    ) {
        let td = TempDir::new().unwrap();
        let mut cur = td.path().to_path_buf();
        for (i, d) in depths.iter().enumerate() {
            let dirname = format!("d{i}");
            cur = cur.join(dirname);
            fs::create_dir_all(&cur).unwrap();
            if with_md.get(i).copied().unwrap_or(false) {
                fs::write(cur.join("AGENTS.md"), format!("rule {i}")).unwrap();
            }
        }
        let r1 = walk_agents_md(&cur);
        let r2 = walk_agents_md(&cur);
        prop_assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            prop_assert_eq!(&a.path, &b.path);
            prop_assert_eq!(&a.contents, &b.contents);
        }
    }

    #[test]
    fn k14_walk_outermost_first(
        depths in prop::collection::vec(1usize..5, 2..6),
    ) {
        let td = TempDir::new().unwrap();
        // Always write root AGENTS.md
        fs::write(td.path().join("AGENTS.md"), "root rule").unwrap();
        let mut cur = td.path().to_path_buf();
        for (i, _) in depths.iter().enumerate() {
            let dirname = format!("d{i}");
            cur = cur.join(dirname);
            fs::create_dir_all(&cur).unwrap();
            fs::write(cur.join("AGENTS.md"), format!("rule {i}")).unwrap();
        }
        let found = walk_agents_md(&cur);
        // Outermost should be td.path(), innermost should be cur
        if !found.is_empty() {
            prop_assert_eq!(&found[0].dir, td.path());
            prop_assert_eq!(&found.last().unwrap().dir, &cur);
        }
    }
}

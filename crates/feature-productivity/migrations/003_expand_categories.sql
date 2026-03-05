-- Expand default category rules with more apps (Ghostty, Linear, etc.)
-- and backfill category_id on existing uncategorized activity events.

-- Update coding category: add Ghostty, Hyper, Rio, WezTerm
UPDATE activity_categories
SET rules = '{"appNames":["Visual Studio Code","Code","Xcode","IntelliJ IDEA","WebStorm","Terminal","iTerm2","Warp","Alacritty","kitty","Ghostty","Hyper","Rio","WezTerm"],"bundleIds":["com.microsoft.VSCode","com.apple.Terminal","com.googlecode.iterm2","com.mitchellh.ghostty"],"urlPatterns":[]}'
WHERE id = 'coding' AND is_system = TRUE;

-- Update communication category: add Signal
UPDATE activity_categories
SET rules = '{"appNames":["Slack","Discord","Telegram","WhatsApp","Messages","Microsoft Teams","Zoom","Signal"],"bundleIds":["com.tinyspeck.slackmacgap","com.hnc.Discord","ru.keepcoder.Telegram"],"urlPatterns":[]}'
WHERE id = 'communication' AND is_system = TRUE;

-- Update browsing category: add Orion, Vivaldi
UPDATE activity_categories
SET rules = '{"appNames":["Safari","Firefox","Google Chrome","Arc","Brave Browser","Orion","Vivaldi"],"bundleIds":["com.apple.Safari","org.mozilla.firefox","com.google.Chrome"],"urlPatterns":[]}'
WHERE id = 'browsing' AND is_system = TRUE;

-- Add project management category (Linear, Jira, Asana, etc.)
INSERT OR IGNORE INTO activity_categories (id, name, category_type, rules, is_system) VALUES
    ('project_management', 'Project Management', 'productive', '{"appNames":["Linear","Jira","Asana","Trello","ClickUp","Monday","Height"],"bundleIds":["com.linear"],"urlPatterns":["linear.app","jira.atlassian.com","asana.com","trello.com"]}', TRUE);

-- Backfill category_id on existing events that match known app names
UPDATE activity_events SET category_id = 'coding'
WHERE category_id IS NULL AND app_name IN ('Visual Studio Code','Code','Xcode','IntelliJ IDEA','WebStorm','Terminal','iTerm2','Warp','Alacritty','kitty','Ghostty','Hyper','Rio','WezTerm');

UPDATE activity_events SET category_id = 'communication'
WHERE category_id IS NULL AND app_name IN ('Slack','Discord','Telegram','WhatsApp','Messages','Microsoft Teams','Zoom','Signal');

UPDATE activity_events SET category_id = 'browsing'
WHERE category_id IS NULL AND app_name IN ('Safari','Firefox','Google Chrome','Arc','Brave Browser','Orion','Vivaldi');

UPDATE activity_events SET category_id = 'project_management'
WHERE category_id IS NULL AND app_name IN ('Linear','Jira','Asana','Trello','ClickUp','Monday','Height');

UPDATE activity_events SET category_id = 'email'
WHERE category_id IS NULL AND app_name IN ('Mail','Spark','Airmail','Superhuman');

UPDATE activity_events SET category_id = 'design'
WHERE category_id IS NULL AND app_name IN ('Figma','Sketch','Adobe Photoshop','Adobe Illustrator','Affinity Designer');

UPDATE activity_events SET category_id = 'documentation'
WHERE category_id IS NULL AND app_name IN ('Notion','Obsidian','Bear','Typora','Pages');

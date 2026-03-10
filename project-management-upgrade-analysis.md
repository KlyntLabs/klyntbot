# Nâng Cấp Hệ Thống Quản Lý Project — Phân Tích & Thiết Kế

## 1. Tổng Quan Hiện Trạng

### 1.1 Kiến Trúc Hiện Tại

Klyntbot hiện đã có nền tảng khá vững với mô hình PARA (Projects, Areas, Resources, Archives) kết hợp OKR (Objectives & Key Results). Hệ thống gồm 26 crates Rust, 9 layers, và một desktop UI React 19 chạy trên Tauri 2.

**Những gì đã có:**

- **Domain models**: Area → Project → Objective → KeyResult → Action (Task)
- **Storage**: SQLite + LanceDB (vectors), 7 migrations, repository pattern
- **Cognitive Memory**: SemanticFact, EpisodicMemory, ProceduralRule, UserModel — hỗ trợ FSRS decay, bi-temporal facts
- **Context Engine**: Pluggable ContextSources (AreaSource, TodoSource, ProductivitySource, IdentitySource, PersonaSource)
- **Agent Runtime**: 5 agent profiles (general, task, finance, automation, communication), IntentAnalyzer, ExecutionRouter
- **Feature Packages**: todo, productivity, finance, coaching, notes — mỗi feature có tools, migrations, config riêng
- **Desktop UI**: Chat with streaming/transparency, Kanban + Table views, OKR pages, Notes (Tiptap + Vim), Finance, Productivity tracking

### 1.2 Những Gì Còn Thiếu

Dựa trên phân tích codebase, các gaps chính cho việc nâng cấp:

- **Không có hệ thống Team/Collaboration** — hoàn toàn single-user
- **Project Context cho AI chưa đủ sâu** — AI biết project tồn tại nhưng không có "project memory" riêng
- **Không có Instructions/Sources per project** — không có cơ chế để user define context cho AI trong từng project
- **Không có Role-based access** — không có khái niệm role trong project
- **Thiếu liên kết chặt giữa features** — tasks, notes, coaching, productivity chưa unified trong project scope

---

## 2. Thiết Kế Hệ Thống Mới: Project-Centric AI Workspace

### 2.1 Triết Lý Thiết Kế

Lấy cảm hứng từ giao diện Grok Projects (Instructions, Sources, Conversations) nhưng đi xa hơn — mỗi project trở thành một **AI workspace** với context riêng, memory riêng, và quan hệ giữa mọi entity trong project.

**Nguyên tắc cốt lõi:**

1. **Project as AI Context Boundary** — Mỗi project là một "thế giới" riêng cho AI, với instructions, knowledge base, và conversation history
2. **Personalized per User per Project** — User có role, goals, và AI coaching riêng trong từng project
3. **Cross-feature Integration** — Tasks, notes, coaching, productivity đều scoped theo project và liên kết với nhau
4. **Memory Hierarchy** — Global memory → Area memory → Project memory → Conversation memory

### 2.2 Data Model Mới

#### 2.2.1 Project Enhancement

```
Project (upgraded)
├── id, name, description, color, icon, area_id
├── status: Active | Paused | Completed | Archived
│
├── ── AI Context Layer ──
├── instructions: Text          # System prompt cho AI khi chat trong project
├── ai_personality: Text        # Tone, style cho AI (formal, casual, technical...)
├── knowledge_base_id: UUID     # Link to project-specific knowledge base
│
├── ── Team Layer ──
├── owner_id: UUID              # Creator/owner
├── visibility: Private | Team | Public
├── default_role: Role
│
├── ── Metadata ──
├── tags: Vec<String>
├── start_date, target_end_date: Option<Date>
├── workflow_id: UUID
├── template_id: Option<UUID>   # Created from template
└── settings: JSON              # Project-specific configs
```

#### 2.2.2 Project Instructions (AI Context)

```
ProjectInstruction
├── id, project_id
├── instruction_type: SystemPrompt | Guideline | Constraint | Persona
├── content: Text               # Markdown content
├── priority: u8                # Ordering khi inject vào AI context
├── active: bool
├── created_at, updated_at
└── created_by: UUID

Ví dụ:
- SystemPrompt: "Bạn đang hỗ trợ project phát triển mobile app. Tech stack: React Native, TypeScript, Firebase."
- Guideline: "Luôn ưu tiên performance và UX. Code review cần pass trước khi merge."
- Constraint: "Budget giới hạn 50M VND. Timeline: 3 tháng."
- Persona: "Communicate bằng tiếng Việt, technical terms giữ tiếng Anh."
```

#### 2.2.3 Project Knowledge Base (Sources)

```
ProjectSource
├── id, project_id
├── source_type: Document | Link | Note | File | Snippet
├── title: String
├── content: Text               # Extracted/raw content
├── url: Option<String>         # For links
├── file_path: Option<String>   # For uploaded files
├── embedding_id: Option<UUID>  # LanceDB vector for RAG
├── metadata: JSON              # File size, type, etc.
├── tags: Vec<String>
├── created_at, updated_at
└── created_by: UUID

Khi user chat trong project context, AI sẽ:
1. Retrieve relevant sources qua semantic search (LanceDB)
2. Inject vào context window với citation
3. Trả lời dựa trên project knowledge + general knowledge
```

#### 2.2.4 Team & Roles

```
TeamMember
├── id
├── user_id: UUID
├── display_name: String
├── email: String
├── avatar: Option<String>
├── status: Active | Invited | Deactivated
└── created_at

ProjectMember
├── id
├── project_id, user_id
├── role: Owner | Admin | Member | Viewer | Custom(String)
├── responsibilities: Text      # Mô tả role cụ thể trong project
├── joined_at
├── ai_coaching_enabled: bool   # Có muốn AI coaching cho role này không
└── custom_instructions: Text   # AI instructions riêng cho member này

Role enum:
- Owner: Full control, delete project, manage members
- Admin: Manage tasks, objectives, settings (không delete project)
- Member: Create/edit tasks, chat, contribute
- Viewer: Read-only access
- Custom: User-defined role với permissions map
```

#### 2.2.5 Project Memory (AI-specific)

```
ProjectMemory
├── id, project_id
├── memory_type: Decision | Insight | Pattern | Preference | Milestone
├── content: Text
├── context: Text               # What triggered this memory
├── importance: f32             # 0.0 - 1.0
├── source_session: Option<String>  # From which conversation
├── source_entity: Option<(EntityKind, UUID)>  # From which task/note/etc
├── tags: Vec<String>
├── created_at
├── last_accessed: DateTime
├── access_count: u32
└── stability: f32              # FSRS decay score

Ví dụ memories:
- Decision: "Team quyết định dùng PostgreSQL thay vì MongoDB cho module billing"
- Insight: "User thường bị stuck ở phần integration testing — cần thêm documentation"
- Pattern: "Sprint velocity trung bình 20 story points/week"
- Preference: "User thích review code vào buổi sáng, creative work buổi chiều"
- Milestone: "v1.0 released on 2026-01-15, 3 weeks ahead of schedule"
```

### 2.3 Project Conversations (Enhanced Chat)

Mỗi project sẽ có nhiều conversations, mỗi conversation có thể linked to entities:

```
ProjectConversation
├── session_key: String         # Existing session system
├── project_id: UUID
├── title: String
├── conversation_type: General | TaskDiscussion | Review | Planning | Retrospective
├── linked_entities: Vec<(EntityKind, UUID)>  # Tasks, objectives, notes...
├── participants: Vec<UUID>     # Team members involved
├── pinned: bool
├── archived: bool
└── created_at, updated_at

Khi AI chat trong project conversation:
1. Load ProjectInstructions → inject as system prompt
2. Retrieve relevant ProjectSources (RAG)
3. Load ProjectMemories (relevant facts + patterns)
4. Load linked entity context (task details, note content...)
5. Apply user's role-specific instructions
6. Conversation → new memories extracted automatically
```

### 2.4 Cross-Feature Integration

#### Tasks within Project Context

```
Action (Task) — enhanced fields:
├── ...existing fields...
├── assigned_to: Option<UUID>       # Team member
├── reviewer: Option<UUID>          # Code/task reviewer
├── linked_notes: Vec<UUID>         # Related notes
├── linked_conversations: Vec<String> # Related chat sessions
└── ai_context: Option<Text>        # AI-generated summary of task context
```

#### Notes ↔ Project

```
Note — enhanced fields:
├── ...existing fields...
├── project_id: Option<UUID>
├── linked_tasks: Vec<UUID>
├── linked_conversations: Vec<String>
└── auto_tagged: bool               # AI auto-categorized
```

#### Coaching per Project

```
ProjectCoachingConfig
├── id, project_id
├── coaching_style: Aggressive | Balanced | Gentle
├── focus_areas: Vec<String>        # ["time_management", "code_quality", "communication"]
├── check_in_frequency: Daily | Weekly | BiWeekly
├── goals: Vec<String>              # What user wants to improve in this project
└── active: bool
```

#### Productivity per Project

```
ProjectProductivitySummary (generated periodically)
├── id, project_id
├── period: Weekly | Monthly | Sprint
├── period_start, period_end
├── tasks_completed, tasks_created
├── focus_hours: f32
├── velocity: f32                   # Story points or task count per period
├── top_blockers: Vec<String>
├── ai_insights: Vec<String>        # AI-generated observations
├── ai_recommendations: Vec<String> # AI-suggested improvements
└── created_at
```

---

## 3. Context Engine Enhancement

### 3.1 New ContextSource: ProjectContextSource

```rust
pub struct ProjectContextSource {
    repos: Repos,
    memory_store: ProjectMemoryStore,
    source_retriever: SourceRetriever, // LanceDB semantic search
}

#[async_trait]
impl ContextSource for ProjectContextSource {
    fn name(&self) -> &str { "project" }
    fn priority(&self) -> u8 { 80 } // Higher than area (75)

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let project_id = ctx.metadata.get("project_id")?;

        // 1. Load project instructions
        let instructions = self.repos.project_instructions
            .list_active(project_id).await;

        // 2. Retrieve relevant sources (RAG)
        let relevant_sources = self.source_retriever
            .search(project_id, &ctx.message_text, top_k: 5).await;

        // 3. Load relevant project memories
        let memories = self.memory_store
            .retrieve_relevant(project_id, &ctx.message_text, top_k: 10).await;

        // 4. Load user's role context
        let member = self.repos.project_members
            .get_by_user(project_id, &ctx.user_id).await;

        // 5. Assemble project context block
        Some(format_project_context(instructions, relevant_sources, memories, member))
    }
}
```

### 3.2 Memory Extraction Pipeline (Enhanced)

Sau mỗi conversation trong project context, hệ thống tự động:

1. **Extract Facts** — LLM phân tích conversation → tạo ProjectMemory entries
2. **Detect Decisions** — Nhận diện decisions ("we decided to...", "let's go with...")
3. **Update Patterns** — Cập nhật productivity patterns, behavior patterns
4. **Link Entities** — Tự động link conversation to relevant tasks/notes
5. **Knowledge Base Update** — Nếu user share new information, tự tạo ProjectSource

### 3.3 Memory Hierarchy

```
Global UserModel (existing cognitive system)
    ├── Identity, Preferences, Energy patterns
    │
    ├── Area-level Memory
    │   ├── Area "Work" → work patterns, tools, team dynamics
    │   │
    │   ├── Project-level Memory (NEW)
    │   │   ├── Project "Mobile App" → decisions, architecture, blockers
    │   │   ├── Project "API Backend" → tech stack, deployment patterns
    │   │   └── ...
    │   │
    │   └── ...
    │
    └── Area "Personal" → personal patterns, goals
```

Khi AI assemble context:
- Luôn include relevant Global memories
- Include Area memories nếu đang trong area context
- Include Project memories nếu đang trong project context
- Priority: Project > Area > Global (project-specific facts override general patterns)

---

## 4. UI/UX Design — Project Detail Page

### 4.1 Layout Mới

Lấy cảm hứng từ Grok Projects nhưng mở rộng:

```
┌─────────────────────────────────────────────────────────┐
│  ← Back   Project Name [color dot] [status badge]      │
│  Area: Work  │  Role: Owner  │  3 members               │
├─────────────┬───────────────────────────────────────────┤
│             │                                           │
│  SIDEBAR    │  MAIN CONTENT                             │
│             │                                           │
│  ⚙ Setup    │  [Tab content based on selection]          │
│    Instructions                                         │
│    Sources  │                                           │
│    Members  │                                           │
│             │                                           │
│  📋 Work     │                                           │
│    Tasks    │                                           │
│    OKRs     │                                           │
│    Notebooks│                                           │
│             │                                           │
│  📊 Insights │                                           │
│    Productivity                                         │
│    Coaching │                                           │
│    Reports  │                                           │
│             │                                           │
│  💬 Chat     │                                           │
│    [thread1]│                                           │
│    [thread2]│                                           │
│    [+ New]  │                                           │
│             │                                           │
└─────────────┴───────────────────────────────────────────┘
```

### 4.2 Các Tab Chi Tiết

**Instructions Tab:**
- Rich text editor (Tiptap) để viết instructions cho AI
- Preset templates: "Software Project", "Research", "Marketing Campaign"
- Toggle active/inactive cho từng instruction block
- Preview: "This is what AI sees when you chat in this project"

**Sources Tab:**
- Upload files (PDF, docs, images)
- Add links (auto-extract content)
- Link existing notes from Notes feature
- Paste code snippets
- Search within sources
- Each source shows: relevance score, last used by AI, access count

**Members Tab:**
- Invite by email
- Assign roles (Owner, Admin, Member, Viewer)
- Per-member: responsibilities description, custom AI instructions
- Activity timeline per member

**Tasks Tab:** (enhanced existing)
- Scoped to project, with assignee avatars
- Dependencies visualization
- Linked conversations per task

**OKRs Tab:** (enhanced existing)
- Progress visualization
- Key Results linked to tasks
- AI-generated progress insights

**Notebooks Tab:**
- Project-scoped notebooks
- Meeting notes, technical docs, research
- Bi-directional links to tasks and conversations

**Productivity Tab:**
- Sprint/period summaries
- Velocity charts
- Focus time breakdown
- AI recommendations

**Coaching Tab:**
- Personalized coaching based on project role
- Behavioral patterns detected
- Improvement suggestions
- Check-in history

**Chat Tab:**
- Project conversations với full AI context
- Conversation types: General, Planning, Review, Retrospective
- AI knows all project context (instructions, sources, memories, tasks, notes)

---

## 5. Implementation Roadmap

### Phase 1: Project Context Foundation (2-3 tuần)

**Backend:**
- Migration 008: `project_instructions`, `project_sources`, `project_memories` tables
- New repos: `ProjectInstructionRepo`, `ProjectSourceRepo`, `ProjectMemoryRepo`
- `ProjectContextSource` cho ContextEngine
- Embed project sources vào LanceDB

**Frontend:**
- Enhanced ProjectDetailPage với tabbed layout
- Instructions editor (Tiptap reuse)
- Sources upload/manage UI
- Project chat scoped conversations

### Phase 2: Team & Roles (2-3 tuần)

**Backend:**
- Migration 009: `team_members`, `project_members` tables
- `TeamMemberRepo`, `ProjectMemberRepo`
- Role-based access checking in handlers
- Per-member AI instructions injection

**Frontend:**
- Members tab UI
- Invite flow
- Role picker
- Activity feed

### Phase 3: Cross-Feature Integration (2-3 tuần)

**Backend:**
- Enhanced Action/Note models với cross-linking fields
- `ProjectCoachingConfig` and coaching scoping
- `ProjectProductivitySummary` generation (scheduled)
- Memory extraction pipeline for project conversations

**Frontend:**
- Enhanced task views with assignees, linked entities
- Project notebooks view
- Productivity dashboard per project
- Coaching panel per project

### Phase 4: AI Intelligence Layer (2-3 tuần)

**Backend:**
- Auto-decision detection in conversations
- Project health scoring algorithm
- Proactive nudges based on project patterns
- Cross-project insights ("You're spending 80% time on Project A, but Project B deadline is closer")

**Frontend:**
- AI insights panel
- Proactive notification system
- Project health dashboard
- Smart suggestions in chat

---

## 6. Ý Tưởng Bổ Sung

### 6.1 Project Templates

Pre-built templates cho common project types:
- **Software Development**: Sprints, code review workflow, CI/CD tracking
- **Research**: Literature review, hypothesis tracking, experiment log
- **Marketing Campaign**: Content calendar, metrics tracking, A/B testing
- **Personal Learning**: Study plan, progress tracking, spaced repetition

Mỗi template bao gồm: pre-configured instructions, suggested sources, task templates, OKR templates, và coaching focus areas.

### 6.2 Project Timeline / Activity Feed

Unified timeline showing:
- Task completions/creations
- Conversation highlights (decisions, milestones)
- Memory captures (AI-detected insights)
- Member activities
- Coaching check-ins
- Productivity milestones

### 6.3 Cross-Project Dashboard

For users working across multiple projects:
- Priority matrix: Urgent × Important across all projects
- Time allocation visualization
- AI recommendation: "Focus on Project B today — deadline approaching"
- Conflict detection: overlapping deadlines, resource contention

### 6.4 AI Project Manager Mode

Một agent profile mới — "Project Manager Agent" — có khả năng:
- Tự động review project health hàng tuần
- Generate sprint reports
- Suggest task prioritization based on OKR progress
- Detect bottlenecks from productivity data
- Facilitate retrospectives with guided questions
- Proactively alert about risks (budget, timeline, scope)

### 6.5 Smart Context Switching

Khi user switch giữa projects, AI tự động:
- Load project context (instructions, memories, recent conversations)
- Adjust tone/personality per project settings
- Show "Where you left off" summary
- Suggest next actions based on priorities

### 6.6 Integration Points

- **Calendar**: Link events to projects, auto-create meeting notes
- **Git**: Link commits to tasks, track development progress (đã có GitSettings)
- **MCP Servers**: Per-project MCP tools (e.g., Google Calendar cho project scheduling)
- **Export**: Project reports as PDF/DOCX for stakeholders

---

## 7. Technical Considerations

### 7.1 Storage Impact

- ProjectMemory sẽ grow nhanh — cần FSRS decay + periodic consolidation
- LanceDB vectors cho sources — tách index per project hay global?
  - Recommend: Global index với project_id filter (simpler, LanceDB handles well)
- Session data cho project conversations — existing session system handles this

### 7.2 Context Window Budget

Với project context mới, token budget cần careful allocation:

```
Total Context Window: ~128K tokens
├── System Prompt (agent persona): ~500 tokens
├── Project Instructions: ~1000 tokens (capped)
├── Project Sources (RAG results): ~2000 tokens (top 3-5 sources)
├── Project Memories: ~500 tokens (top 10 facts)
├── User Role Context: ~200 tokens
├── Area Context: ~300 tokens
├── Global User Model: ~300 tokens
├── Conversation History: ~4000-8000 tokens
├── Tool Definitions: ~2000 tokens
└── Reserved for response: ~4000 tokens
```

Existing BudgetAllocator đã handle dynamic allocation — chỉ cần thêm ProjectContextSource với budget hints.

### 7.3 Migration Strategy

Backward compatible — tất cả new fields đều optional. Existing projects continue to work. Users opt-in to enhanced features by adding instructions/sources/members.

### 7.4 Performance

- Project context cache: TTL-based, invalidate on instruction/source changes
- Source embedding: Async job khi upload (không block UI)
- Memory extraction: Background task sau mỗi conversation
- Productivity summaries: Scheduled cron (existing scheduling crate)

---

## 8. Kết Luận

Hệ thống hiện tại của Klyntbot đã có nền tảng rất tốt — PARA framework, cognitive memory, pluggable context engine, và feature package architecture. Việc nâng cấp project management cần tập trung vào 3 trụ cột:

1. **Project as AI Context** — Instructions, Sources, Memories scoped per project
2. **Team Collaboration** — Members, Roles, Shared conversations
3. **Unified Intelligence** — Cross-feature integration, proactive coaching, smart insights

Cách tiếp cận phased (4 phases, mỗi phase 2-3 tuần) cho phép ship incremental value. Phase 1 (Project Context) là critical nhất và tạo foundation cho tất cả phases sau.

Điểm khác biệt lớn nhất so với Grok Projects hay các tool khác: **AI không chỉ chat trong project context, mà thực sự "sống" trong project** — ghi nhớ decisions, học patterns, proactively coaching, và connecting dots giữa tasks, notes, conversations, và productivity data.

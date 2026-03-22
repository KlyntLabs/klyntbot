// tools/simulator/modules/notes.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";
import { pick } from "../utils/random";

interface CreateResponse { id: string; [key: string]: unknown }

export const notesModule: SimulatorModule = {
    name: "notes",
    description: "Notebooks, notes with @mentions and [[wikilinks]]",
    dependencies: ["para", "tasks"],

    async seed(world, client) {
        // Create 3 notebooks
        const workRes = await client.post<CreateResponse>("notebook_create", {
            title: "Work Research",
            icon: "🔬",
        });
        world.notebooks.workResearch = { id: workRes.id, title: "Work Research" };

        const studyRes = await client.post<CreateResponse>("notebook_create", {
            title: "Study Notes",
            icon: "📚",
        });
        world.notebooks.studyNotes = { id: studyRes.id, title: "Study Notes" };

        const journalRes = await client.post<CreateResponse>("notebook_create", {
            title: "Daily Journal",
            icon: "📓",
        });
        world.notebooks.dailyJournal = { id: journalRes.id, title: "Daily Journal" };

        console.log(`  3 notebooks created`);
    },

    async simulateDay(world, client, day) {
        switch (day.dayOfWeek) {
            case 0: // Monday — meeting notes with @task mentions + inbox captures
                await createMeetingNote(world, client, day);
                await createInboxItems(client);
                break;
            case 1: // Tuesday — research notes with wikilinks
                await createResearchNote(world, client, day);
                break;
            case 2: // Wednesday — retrospective with @project mentions + version snapshot
                await createRetroNote(world, client, day);
                await snapshotRecentNote(world, client, "api-retro");
                break;
            case 3: // Thursday — study notes + annotation
                await createStudyNote(world, client, day);
                await annotateStudyNote(world, client);
                await snapshotRecentNote(world, client, "oauth-patterns");
                break;
            case 4: // Friday — journal entry
                await createJournalNote(world, client, day);
                break;
            case 5: // Saturday — light personal + version snapshot
                await createPersonalNote(world, client, day);
                await snapshotRecentNote(world, client, "auth-meeting-notes");
                break;
            case 6: // Sunday — light personal
                await createWeeklyReflection(world, client, day);
                break;
        }
    },
};

async function createMeetingNote(world: World, client: ApiClient, day: DayContext): Promise<void> {
    const authTask = world.createdTasks.get("auth-layer");
    const jwtTask = world.createdTasks.get("jwt-refresh");
    const taskMentions = [
        authTask ? `@task:${authTask.id}` : "",
        jwtTask ? `@task:${jwtTask.id}` : "",
    ].filter(Boolean).join("\n- ");

    const body = `# Auth Design Meeting Notes

**Date:** ${day.date.toISOString().split("T")[0]}
**Attendees:** Sarah, Mike, Jay

## Discussion

Reviewed the current auth implementation and discussed migration strategy.

## Action Items
- ${taskMentions}
- Review token rotation strategy by Wednesday
- Set up staging environment for auth testing

## Decisions
- Will use JWT with short-lived access tokens (15min) and refresh tokens (7d)
- Rate limiting will be IP-based initially, upgrade to user-based in phase 2

@project:${world.projects.apiRedesign.id}`;

    const res = await client.post<CreateResponse>("note_create", {
        title: "Auth Design Meeting Notes",
        notebookId: world.notebooks.workResearch.id,
        body,
        tags: ["meeting", "auth", "api-redesign"],
    });
    world.createdNotes.set("auth-meeting-notes", { id: res.id, title: "Auth Design Meeting Notes" });
    console.log(`  notes: created "Auth Design Meeting Notes" with @task mentions`);
}

async function createResearchNote(world: World, client: ApiClient, day: DayContext): Promise<void> {
    // Build wikilinks to existing notes
    const existingNotes = [...world.createdNotes.values()];
    const wikilinks = existingNotes
        .slice(0, 3)
        .map(n => `[[${n.title}]]`)
        .join(", ");

    const body = `# Paris Trip Budget Analysis

## Overview
Estimated total budget for 7-day Paris trip: $3,500-$4,500

## Breakdown
| Category | Estimate |
|----------|----------|
| Flights | $600-$900 |
| Hotel (7 nights) | $1,200-$1,800 |
| Food & Dining | $500-$700 |
| Activities | $200-$400 |
| Transport | $100-$200 |
| Shopping | $300-$500 |

## Notes
- Check credit card travel benefits for lounge access
- Book museum passes in advance (Paris Museum Pass)
- Consider Airbnb for better kitchen access

## Related
See also: ${wikilinks || "no related notes yet"}

@project:${world.projects.parisTrip.id}`;

    const res = await client.post<CreateResponse>("note_create", {
        title: "Paris Trip Budget Analysis",
        notebookId: world.notebooks.workResearch.id,
        body,
        tags: ["travel", "paris", "budget"],
    });
    world.createdNotes.set("paris-budget-analysis", { id: res.id, title: "Paris Trip Budget Analysis" });
    console.log(`  notes: created "Paris Trip Budget Analysis" with [[wikilinks]]`);
}

async function createRetroNote(world: World, client: ApiClient, day: DayContext): Promise<void> {
    const authMeeting = world.createdNotes.get("auth-meeting-notes");
    const wikilink = authMeeting ? `[[${authMeeting.title}]]` : "";

    const body = `# API Redesign Sprint Retrospective

**Sprint:** Week of ${day.date.toISOString().split("T")[0]}

## What went well
- Auth layer implementation progressing ahead of schedule
- Good collaboration during design review (see ${wikilink})
- Test coverage improved to 85%

## What could improve
- Need better staging deployment automation
- Code review turnaround time could be faster
- Documentation is falling behind

## Action items
- Set up automated staging deploys by next sprint
- Establish 24h code review SLA
- Dedicate Thursday afternoons to documentation

@project:${world.projects.apiRedesign.id}`;

    const res = await client.post<CreateResponse>("note_create", {
        title: "API Redesign Sprint Retrospective",
        notebookId: world.notebooks.workResearch.id,
        body,
        tags: ["retro", "api-redesign", "sprint"],
    });
    world.createdNotes.set("api-retro", { id: res.id, title: "API Redesign Sprint Retrospective" });
    console.log(`  notes: created retrospective with @project mention`);
}

async function createStudyNote(world: World, client: ApiClient, day: DayContext): Promise<void> {
    const body = `# OAuth 2.0 Patterns and Best Practices

## Authorization Code Flow
The authorization code flow is the most secure OAuth 2.0 flow for server-side applications.

1. Client redirects user to authorization server
2. User authenticates and grants permissions
3. Authorization server returns code to redirect URI
4. Client exchanges code for tokens (server-to-server)

## Key Concepts
- **Access Token:** Short-lived, used for API requests
- **Refresh Token:** Long-lived, used to obtain new access tokens
- **PKCE:** Proof Key for Code Exchange — prevents authorization code interception
- **Scopes:** Define the permissions granted to the application

## Security Considerations
- Always use HTTPS for token exchange
- Store refresh tokens securely (encrypted at rest)
- Implement token rotation for refresh tokens
- Use short expiry times for access tokens (15-30 min)

## Related
See also the auth implementation: @task:${world.createdTasks.get("auth-layer")?.id ?? "unknown"}`;

    const res = await client.post<CreateResponse>("note_create", {
        title: "OAuth 2.0 Patterns",
        notebookId: world.notebooks.studyNotes.id,
        body,
        tags: ["study", "oauth", "security", "authentication"],
    });
    world.createdNotes.set("oauth-patterns", { id: res.id, title: "OAuth 2.0 Patterns" });
    console.log(`  notes: created "OAuth 2.0 Patterns" study note`);
}

async function createJournalNote(world: World, client: ApiClient, day: DayContext): Promise<void> {
    const existingNotes = [...world.createdNotes.values()];
    const recentLinks = existingNotes
        .slice(-3)
        .map(n => `[[${n.title}]]`)
        .join("\n- ");

    const body = `# Weekly Wrap-Up Journal

**Date:** ${day.date.toISOString().split("T")[0]}

## Accomplishments
- Made good progress on the API redesign auth layer
- Paris trip planning is coming together — budget looks reasonable
- Kept up with French vocabulary practice

## Reflections
- Need to be more disciplined about deep work blocks
- The morning routine is working well for focus sessions
- Should spend more time on the FIRE goal tracking

## Next Week Focus
- Complete JWT refresh token implementation
- Book Paris flights (prices are going up)
- Hit 30min French practice daily

## References
- ${recentLinks || "no references"}

## Mood: 😊 Productive, slightly tired`;

    const res = await client.post<CreateResponse>("note_create", {
        title: "Weekly Wrap-Up",
        notebookId: world.notebooks.dailyJournal.id,
        body,
        tags: ["journal", "weekly", "reflection"],
    });
    world.createdNotes.set(`journal-day${day.dayIndex}`, { id: res.id, title: "Weekly Wrap-Up" });
    console.log(`  notes: created journal entry with [[wikilinks]]`);
}

async function createPersonalNote(world: World, client: ApiClient, day: DayContext): Promise<void> {
    const body = `# French Vocabulary — Week ${Math.floor(day.dayIndex / 7) + 1}

## New Words
| French | English | Example |
|--------|---------|---------|
| quotidien | daily | La vie quotidienne |
| davantage | more | Je veux en savoir davantage |
| environ | about/around | Il y a environ 30 personnes |
| auparavant | beforehand | Je l'ai vu auparavant |
| cependant | however | Cependant, il fait beau |

## Grammar Note
The subjunctive mood (subjonctif) is used after expressions of doubt, desire, and emotion:
- Il faut que tu **sois** prudent
- Je veux que nous **allions** au cinéma

## Practice Sentences
1. Il est important que je pratique quotidiennement.
2. Cependant, je n'ai pas eu le temps auparavant.

@project:${world.projects.languageLearning.id}`;

    const res = await client.post<CreateResponse>("note_create", {
        title: `French Vocab - Week ${Math.floor(day.dayIndex / 7) + 1}`,
        notebookId: world.notebooks.studyNotes.id,
        body,
        tags: ["french", "vocabulary", "learning"],
    });
    world.createdNotes.set(`french-vocab-day${day.dayIndex}`, { id: res.id, title: `French Vocab - Week ${Math.floor(day.dayIndex / 7) + 1}` });
    console.log(`  notes: created French vocabulary note`);
}

async function createWeeklyReflection(world: World, client: ApiClient, day: DayContext): Promise<void> {
    const allNotes = [...world.createdNotes.values()];
    const recentLinks = allNotes
        .slice(-5)
        .map(n => `- [[${n.title}]]`)
        .join("\n");

    const body = `# Sunday Planning & Reflection

**Week of:** ${day.date.toISOString().split("T")[0]}

## This Week's Highlights
- Auth layer implementation progressed well
- Paris trip budget research completed
- French vocab practice maintained daily streak

## Areas for Improvement
- Didn't hit 4h deep work goal on 2 days
- Finance review got pushed — need to make it non-negotiable
- Sleep schedule slipped on Thursday

## Next Week Intentions
1. Complete auth endpoint migration
2. Book Paris flights
3. Review FIRE goal monthly numbers
4. Hit 4h deep work target every weekday

## Connected Notes
${recentLinks || "No connected notes yet"}`;

    const res = await client.post<CreateResponse>("note_create", {
        title: "Sunday Planning & Reflection",
        notebookId: world.notebooks.dailyJournal.id,
        body,
        tags: ["journal", "planning", "weekly"],
    });
    world.createdNotes.set(`reflection-day${day.dayIndex}`, { id: res.id, title: "Sunday Planning & Reflection" });
    console.log(`  notes: created Sunday planning note`);
}

// ── Note versions, annotations, inbox ───────────────────────────────

async function snapshotRecentNote(world: World, client: ApiClient, noteKey: string): Promise<void> {
    const note = world.createdNotes.get(noteKey);
    if (!note) return;
    try {
        await client.postFlat("note_version_create", { note_id: note.id });
        console.log(`  notes: version snapshot for "${note.title}"`);
    } catch {
        // Version creation may fail if note doesn't exist yet
    }
}

async function annotateStudyNote(world: World, client: ApiClient): Promise<void> {
    const note = world.createdNotes.get("oauth-patterns");
    if (!note) return;
    try {
        await client.post("annotation_create", {
            noteId: note.id,
            markId: crypto.randomUUID(),
            content: "Key concept — remember this for the auth migration",
            quotedText: "OAuth 2.0",
        });
        await client.post("annotation_create", {
            noteId: note.id,
            markId: crypto.randomUUID(),
            content: "Compare with our current implementation in the API redesign",
            quotedText: "PKCE",
        });
        console.log(`  notes: 2 annotations added to study note`);
    } catch {
        // Annotation creation may fail
    }
}

async function createInboxItems(client: ApiClient): Promise<void> {
    await client.post("inbox_create", {
        content: "Look into rate limiting libraries for the API",
    });
    await client.post("inbox_create", {
        content: "Sarah mentioned a good French podcast — check it out",
    });
    await client.post("inbox_create", {
        content: "Read up on PKCE flow for mobile OAuth clients",
    });
    console.log(`  notes: 3 inbox items captured`);
}

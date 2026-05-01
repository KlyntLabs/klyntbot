import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function PermissionsSubsection() {
  const [allow, setAllow] = useState("");
  const [deny, setDeny] = useState("");
  const [ask, setAsk] = useState("");

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as {
        permissions?: { allow?: string[]; deny?: string[]; ask?: string[] };
      };
      setAllow((cfg.permissions?.allow ?? []).join("\n"));
      setDeny((cfg.permissions?.deny ?? []).join("\n"));
      setAsk((cfg.permissions?.ask ?? []).join("\n"));
    })();
  }, []);

  return (
    <section>
      <h3>Layer-1 declarative rules</h3>
      <p>
        One pattern per line. Patterns match command prefixes — see{" "}
        <code>~/.klyntbot/rules/*.rules</code> for the Layer-2 Starlark equivalent.
      </p>
      <label>
        Allow <textarea value={allow} onChange={(e) => setAllow(e.target.value)} rows={4} />
      </label>
      <label>
        Deny <textarea value={deny} onChange={(e) => setDeny(e.target.value)} rows={4} />
      </label>
      <label>
        Ask <textarea value={ask} onChange={(e) => setAsk(e.target.value)} rows={4} />
      </label>
      <button
        type="button"
        onClick={() =>
          invoke("config_set_coding_permissions", {
            allow: allow.split("\n").filter(Boolean),
            deny: deny.split("\n").filter(Boolean),
            ask: ask.split("\n").filter(Boolean),
          })
        }
      >
        Save
      </button>
    </section>
  );
}

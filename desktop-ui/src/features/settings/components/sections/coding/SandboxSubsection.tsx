import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

type CodingSection = { sandbox?: { enforce?: boolean } };

export function SandboxSubsection() {
  const [enforce, setEnforce] = useState(true);
  const [testResult, setTestResult] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const cfg = (await invoke("config_get_section", { section: "coding" })) as CodingSection;
        setEnforce(cfg.sandbox?.enforce ?? true);
      } catch (e) {
        console.warn("[SandboxSubsection] load failed", e);
      }
    })();
  }, []);

  const save = () =>
    invoke("config_update_section", {
      section: "coding",
      patch: { sandbox: { enforce } },
    });

  const test = async () => {
    setTestResult("Testing…");
    try {
      const result = (await invoke("coding_test_sandbox")) as { ok: boolean; details: string };
      setTestResult(result.ok ? `OK: ${result.details}` : `Failed: ${result.details}`);
    } catch (e) {
      setTestResult(`Error: ${e}`);
    }
  };

  return (
    <section>
      <label>
        <input type="checkbox" checked={enforce} onChange={(e) => setEnforce(e.target.checked)} />
        Enforce sandbox for all tool execution
      </label>
      {!enforce && (
        <p className="warn">
          Disabling the sandbox lets bash run unconfined. For pentesting / dev only.
        </p>
      )}
      <button type="button" onClick={save}>
        Save
      </button>
      <button type="button" onClick={test}>
        Test sandbox
      </button>
      {testResult && <p>{testResult}</p>}
    </section>
  );
}

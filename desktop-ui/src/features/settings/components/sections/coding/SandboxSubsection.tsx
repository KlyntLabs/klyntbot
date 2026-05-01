import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function SandboxSubsection() {
  const [enforce, setEnforce] = useState(true);
  const [testResult, setTestResult] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as {
        sandbox?: { enforce?: boolean };
      };
      setEnforce(cfg.sandbox?.enforce ?? true);
    })();
  }, []);

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
      <button type="button" onClick={() => invoke("config_set_coding_sandbox", { enforce })}>
        Save
      </button>
      <button type="button" onClick={test}>
        Test sandbox
      </button>
      {testResult && <p>{testResult}</p>}
    </section>
  );
}

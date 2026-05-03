import { useEffect, useState } from "react";
import { providersList, providerStatus } from "@/api/endpoints/providers";
import type { ProviderListItem, ProviderStatusResult } from "@/api/endpoints/providers";

export function ProvidersSubsection() {
  const [providers, setProviders] = useState<ProviderListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<Record<string, ProviderStatusResult>>({});

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const result = await providersList();
        if (mounted) {
          setProviders(result.providers);
          setLoading(false);
        }
      } catch (e) {
        if (mounted) {
          setError(e instanceof Error ? e.message : "Failed to load providers");
        }
      } finally {
        if (mounted) setLoading(false);
      }
    })();
    return () => {
      mounted = false;
    };
  }, []);

  const handleTest = async (providerId: string) => {
    setTestingId(providerId);
    try {
      const result = await providerStatus(providerId);
      setTestResult((prev) => ({ ...prev, [providerId]: result }));
    } catch (e) {
      setTestResult((prev) => ({
        ...prev,
        [providerId]: { id: providerId, available: false, error: e instanceof Error ? e.message : "Test failed" },
      }));
    } finally {
      setTestingId(null);
    }
  };

  if (loading) {
    return <section className="providers-subsection"><p>Loading providers…</p></section>;
  }

  if (error) {
    return <section className="providers-subsection"><p className="providers-subsection__error">{error}</p></section>;
  }

  return (
    <section className="providers-subsection">
      <h3>Providers</h3>
      {providers.length === 0 ? (
        <p className="providers-subsection__empty">No providers configured.</p>
      ) : (
        <ul className="providers-subsection__list">
          {providers.map((p) => (
            <li key={p.id} className="providers-subsection__item">
              <div className="providers-subsection__item-info">
                <span className="providers-subsection__name">{p.name}</span>
                <span className="providers-subsection__status">
                  {p.hasApiKey ? "✓ API key set" : "✗ No API key"}
                </span>
                {p.defaultModel && (
                  <span className="providers-subsection__model">Default: {p.defaultModel}</span>
                )}
              </div>
              <div className="providers-subsection__actions">
                <button
                  type="button"
                  className="providers-subsection__test-btn"
                  onClick={() => handleTest(p.id)}
                  disabled={testingId === p.id || !p.hasApiKey}
                >
                  {testingId === p.id ? "Testing…" : "Test"}
                </button>
                {(() => {
                  const r = testResult[p.id];
                  if (!r) return null;
                  return (
                    <span
                      className={`providers-subsection__test-result ${r.available ? "success" : "error"}`}
                    >
                      {r.available ? "OK" : r.error}
                    </span>
                  );
                })()}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

import { useEffect, useState } from "react";
import type { Go } from "../App";
import { errorText, scanApps, type ScannedApp } from "../api";
import { Loading, Notice, Screen, Steps } from "../components";
import { count } from "../format";

export const SAVE_STEPS = ["Find apps", "Choose apps", "Check and save"];

/** Looks for the apps we know how to move. Reads only. */
export default function Scan(props: { go: Go }) {
  const [apps, setApps] = useState<ScannedApp[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = () => {
    setApps(null);
    setError(null);
    scanApps().then(setApps, (err) => setError(errorText(err)));
  };
  useEffect(run, []);

  const found = apps?.filter((app) => app.status === "found") ?? [];
  const missing = apps?.filter((app) => app.status !== "found") ?? [];

  return (
    <Screen
      title="Finding your apps"
      onBack={() => props.go({ name: "home" })}
      footer={
        apps && (
          <>
            <button className="secondary" onClick={run}>
              Look again
            </button>
            <button
              className="primary"
              disabled={found.length === 0}
              onClick={() => props.go({ name: "choose", apps })}
            >
              Next: choose apps
            </button>
          </>
        )
      }
    >
      <Steps steps={SAVE_STEPS} current={0} />
      {error && <Notice tone="error" title="Couldn't look for apps">{error}</Notice>}
      {!apps && !error && <Loading>Looking for apps on this computer…</Loading>}
      {apps && (
        <>
          <p className="lead">
            {found.length === 0
              ? "None of the apps we know how to move have settings on this computer yet."
              : `Found settings for ${count(found.length, "app")} of the ${apps.length} we know how to move.`}
          </p>
          <ul className="app-list">
            {found.map((app) => (
              <li key={app.id} className="app-row">
                <span className="mark found" aria-label="Found">✓</span>
                <span className="app-name">{app.name}</span>
                <span className="muted">{count(app.files.length, "settings file")}</span>
              </li>
            ))}
            {missing.map((app) => (
              <li key={app.id} className="app-row missing">
                <span className="mark" aria-label="Not found">–</span>
                <span className="app-name">{app.name}</span>
                <span className="muted">
                  {app.status === "unsupported" ? "Not supported on this computer" : "Not on this computer"}
                </span>
              </li>
            ))}
          </ul>
        </>
      )}
    </Screen>
  );
}

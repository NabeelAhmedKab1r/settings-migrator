// Capture: shows what will be saved, asks about anything that looks like a
// password or key (the secret review list), then writes the settings file
// where the user chooses. Nothing is written before "Save".

import { useEffect, useState } from "react";
import type { Go } from "../App";
import {
  errorText,
  pickSavePath,
  planCapture,
  saveBundle,
  type CaptureFile,
  type ScannedApp,
} from "../api";
import { Loading, Notice, Screen, Steps } from "../components";
import { bundleName, capitalize, count } from "../format";
import { SAVE_STEPS } from "./Scan";

type Decision = "skip" | "allow";

export default function Capture(props: { go: Go; apps: ScannedApp[]; chosen: string[] }) {
  const [files, setFiles] = useState<CaptureFile[] | null>(null);
  const [decisions, setDecisions] = useState<Record<string, Decision>>({});
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<{ path: string; files: number } | null>(null);

  const load = () => {
    setFiles(null);
    setError(null);
    planCapture(props.chosen).then(
      (planned) => {
        setFiles(planned);
        // Leaving a flagged file out is the safe choice, so it's the default.
        const initial: Record<string, Decision> = {};
        for (const file of planned) if (file.findings.length > 0) initial[file.key] = "skip";
        setDecisions(initial);
      },
      (err) => setError(errorText(err)),
    );
  };
  useEffect(load, [props.chosen]);

  const flagged = files?.filter((file) => file.findings.length > 0) ?? [];
  const included = files?.filter((file) => decisions[file.key] !== "skip") ?? [];
  const apps = new Set(included.map((file) => file.app_id));

  const onSave = async () => {
    setError(null);
    const path = await pickSavePath(bundleName());
    if (!path) return;
    setSaving(true);
    const keys = (decision: Decision) =>
      Object.keys(decisions).filter((key) => decisions[key] === decision);
    try {
      setSaved(await saveBundle(props.chosen, keys("skip"), keys("allow"), path));
    } catch (err) {
      setError(errorText(err));
    } finally {
      setSaving(false);
    }
  };

  if (saved) {
    const name = saved.path.split(/[\\/]/).pop();
    return (
      <Screen title="Your settings are saved">
        <Notice tone="success">
          Saved {count(saved.files, "settings file")} into <strong>{name}</strong>.
        </Notice>
        <h2>What next</h2>
        <ol className="next-steps">
          <li>Copy {name} to your new computer: a USB stick, a cloud drive or email all work.</li>
          <li>Install Settings Migrator there and open it.</li>
          <li>Choose <strong>Put settings on this computer</strong> and pick the file.</li>
        </ol>
        <p className="muted small">
          The file has no passwords or sign-ins, but it does have your settings, so keep it
          to yourself. It was saved to {saved.path}.
        </p>
        <button className="primary" onClick={() => props.go({ name: "home" })}>
          Done
        </button>
      </Screen>
    );
  }

  return (
    <Screen
      title="Check and save"
      onBack={() => props.go({ name: "choose", apps: props.apps, chosen: props.chosen })}
      footer={
        files && (
          <button
            className="primary"
            disabled={saving || included.length === 0}
            onClick={onSave}
          >
            {saving ? "Saving…" : `Save ${count(included.length, "file")}…`}
          </button>
        )
      }
    >
      <Steps steps={SAVE_STEPS} current={2} />
      {error && <Notice tone="error" title="Not saved">{error}</Notice>}
      {!files && !error && <Loading>Reading your settings…</Loading>}

      {flagged.length > 0 && (
        <section className="review" aria-labelledby="review-title">
          <h2 id="review-title">
            {flagged.length === 1
              ? "1 file might hold a password or key"
              : `${flagged.length} files might hold a password or key`}
          </h2>
          <p>
            Passwords and keys should never leave this computer. We've found something that
            looks like one in the files below. Unless you're sure it's harmless, leave the
            file out.
          </p>
          {flagged.map((file) => (
            <div key={file.key} className="review-item">
              <div className="review-file">
                <strong>{file.app_name}</strong> · {file.path}
              </div>
              <ul className="findings">
                {file.findings.map((finding, i) => (
                  <li key={i}>
                    Line {finding.line}: {capitalize(finding.kind)} <code>{finding.excerpt}</code>
                  </li>
                ))}
              </ul>
              <fieldset className="decision">
                <legend className="sr-only">What to do with {file.path}</legend>
                <label>
                  <input
                    type="radio"
                    name={file.key}
                    checked={decisions[file.key] === "skip"}
                    onChange={() => setDecisions({ ...decisions, [file.key]: "skip" })}
                  />
                  Leave this file out <span className="muted">(safest)</span>
                </label>
                <label>
                  <input
                    type="radio"
                    name={file.key}
                    checked={decisions[file.key] === "allow"}
                    onChange={() => setDecisions({ ...decisions, [file.key]: "allow" })}
                  />
                  Include it: I've checked, it's not a password or key
                </label>
              </fieldset>
            </div>
          ))}
        </section>
      )}

      {files && (
        <section>
          <h2>What will be saved</h2>
          {included.length === 0 ? (
            <p>Nothing is left to save. Go back and choose more apps.</p>
          ) : (
            <p>
              {count(included.length, "settings file")} from {count(apps.size, "app")}. You'll
              choose where to save it next.
            </p>
          )}
          <ul className="app-list">
            {props.chosen.map((id) => {
              const mine = files.filter((file) => file.app_id === id);
              const left = mine.filter((file) => decisions[file.key] !== "skip");
              const name = props.apps.find((app) => app.id === id)?.name ?? id;
              return (
                <li key={id} className="app-row">
                  <span className="app-name">{name}</span>
                  <span className="muted">
                    {count(left.length, "file")}
                    {left.length < mine.length && `, ${mine.length - left.length} left out`}
                  </span>
                </li>
              );
            })}
          </ul>
        </section>
      )}
    </Screen>
  );
}

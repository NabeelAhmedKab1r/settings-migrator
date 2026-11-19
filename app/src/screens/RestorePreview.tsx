// Restore preview: pick a settings file, see every change file by file
// (with diffs, what was adjusted for this OS, and what couldn't be), then
// restore. Originals are backed up first, so it can be undone.

import { useState } from "react";
import type { Go } from "../App";
import {
  errorText,
  pickBundle,
  previewRestore,
  restore,
  type RestoreFile,
  type RestorePreview as Preview,
} from "../api";
import { DiffView, Loading, Message, Notice, QuitApps, Screen } from "../components";
import { count, when } from "../format";

const writes = (file: RestoreFile) => file.action === "create" || file.action === "update";

export default function RestorePreview(props: { go: Go }) {
  const [bundle, setBundle] = useState<string | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [chosen, setChosen] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<{ files: number; flags: number } | null>(null);

  const load = async (path: string, keepChoice = false) => {
    setLoading(true);
    try {
      const next = await previewRestore(path);
      setBundle(path);
      setPreview(next);
      if (!keepChoice) {
        setChosen(next.apps.filter((app) => app.files.some(writes)).map((app) => app.id));
      }
    } catch (err) {
      setError(errorText(err));
    } finally {
      setLoading(false);
    }
  };

  const pick = async () => {
    setError(null);
    const path = await pickBundle();
    if (path) await load(path);
  };

  const onRestore = async () => {
    if (!bundle || !preview) return;
    setError(null);
    setRestoring(true);
    try {
      const result = await restore(bundle, chosen);
      const flags = preview.apps
        .filter((app) => chosen.includes(app.id))
        .flatMap((app) => app.files.filter(writes))
        .reduce((sum, file) => sum + file.flags.length, 0);
      setDone({ files: result.files, flags });
    } catch (err) {
      setError(errorText(err));
      // Show what's true now, such as a file changed since the preview.
      await load(bundle, true);
    } finally {
      setRestoring(false);
    }
  };

  if (done) {
    return (
      <Screen title="Your settings are in place">
        <Notice tone="success">
          Restored {count(done.files, "settings file")}. Open your apps to see them.
        </Notice>
        {done.flags > 0 && (
          <Notice tone="warning">
            {done.flags === 1 ? "1 thing" : `${done.flags} things`} couldn't be adjusted for
            this computer and {done.flags === 1 ? "was" : "were"} put back as{" "}
            {done.flags === 1 ? "it was" : "they were"}. Have a look at{" "}
            {done.flags === 1 ? "it" : "them"} in the app.
          </Notice>
        )}
        <p>Your earlier settings were saved first. If something isn't right, you can undo this.</p>
        <div className="row">
          <button className="primary" onClick={() => props.go({ name: "home" })}>
            Done
          </button>
          <button className="secondary" onClick={() => props.go({ name: "undo" })}>
            Undo this
          </button>
        </div>
      </Screen>
    );
  }

  if (!preview) {
    return (
      <Screen
        title="Put settings on this computer"
        intro="Choose the settings file you saved on your other computer. Nothing changes until you've seen exactly what will happen."
        onBack={() => props.go({ name: "home" })}
      >
        {error && <Notice tone="error" title="Couldn't open that file">{error}</Notice>}
        {loading ? (
          <Loading>Reading the settings file…</Loading>
        ) : (
          <button className="primary big" onClick={pick}>
            Choose settings file…
          </button>
        )}
      </Screen>
    );
  }

  const chosenApps = preview.apps.filter((app) => chosen.includes(app.id));
  const toWrite = chosenApps.flatMap((app) => app.files.filter(writes));
  const toCheck = toWrite.reduce((sum, file) => sum + file.flags.length, 0);
  const running = preview.running.filter((name) =>
    chosenApps.some((app) => app.name === name),
  );
  const toggle = (id: string) =>
    setChosen(chosen.includes(id) ? chosen.filter((c) => c !== id) : [...chosen, id]);

  return (
    <Screen
      title="Check the changes"
      intro={
        <>
          From <strong>{preview.bundle_name}</strong>, saved on {preview.source_os} on{" "}
          {when(preview.created)}.
        </>
      }
      onBack={() => {
        setPreview(null);
        setBundle(null);
        setError(null);
      }}
      footer={
        <>
          <span className="muted small footer-note">
            Your current settings are saved first, so you can undo this.
          </span>
          <button
            className="primary"
            disabled={restoring || loading || toWrite.length === 0 || running.length > 0}
            onClick={onRestore}
          >
            {restoring ? "Restoring…" : `Restore ${count(toWrite.length, "file")}`}
          </button>
        </>
      }
    >
      {error && <Notice tone="error" title="Nothing was restored">{error}</Notice>}
      {preview.cross_os && (
        <Notice tone="info" title={`Moving from ${preview.source_os} to ${preview.target_os}`}>
          Shortcuts, folder locations and line endings have been adjusted for{" "}
          {preview.target_os}.
          {toCheck > 0 &&
            ` ${toCheck === 1 ? "1 thing" : `${toCheck} things`} couldn't be adjusted and ${toCheck === 1 ? "is" : "are"} marked below; ${toCheck === 1 ? "it's" : "they're"} put back as ${toCheck === 1 ? "it was" : "they were"}.`}
        </Notice>
      )}
      {running.length > 0 && (
        <QuitApps
          apps={running}
          doing="the restore"
          onCheck={() => bundle && load(bundle, true)}
        />
      )}
      {preview.apps.every((app) => !app.files.some(writes)) && (
        <Notice tone="success">Everything already matches. There's nothing to change.</Notice>
      )}

      {preview.apps.map((app) => {
        const changing = app.files.filter(writes);
        const included = chosen.includes(app.id);
        return (
          <section key={app.id} className={`app-card ${included ? "" : "excluded"}`}>
            <label className="check app-card-title">
              <input
                type="checkbox"
                checked={included}
                disabled={changing.length === 0}
                onChange={() => toggle(app.id)}
              />
              <span className="app-name">{app.name}</span>
              <span className="muted small">
                {changing.length === 0 ? "nothing to change" : count(changing.length, "file") + " to change"}
              </span>
            </label>
            {included && (
              <ul className="restore-files">
                {app.files.map((file) => (
                  <FileRow key={file.path} file={file} />
                ))}
              </ul>
            )}
          </section>
        );
      })}
    </Screen>
  );
}

const LABELS: Record<RestoreFile["action"], string> = {
  create: "New",
  update: "Replaces yours",
  unchanged: "Already the same",
  skip: "Skipped",
};

function FileRow(props: { file: RestoreFile }) {
  const { file } = props;
  return (
    <li className={`restore-file ${file.action}`}>
      <div className="restore-file-head">
        <span className={`badge ${file.action}`}>{LABELS[file.action]}</span>
        <span className="file-path" title={file.target ?? undefined}>
          {file.path}
        </span>
      </div>
      {file.skip_reason && (
        <p className="muted small">
          Not put back: <Message text={file.skip_reason} />.
        </p>
      )}
      {file.changes.length > 0 && (
        <ul className="changes">
          {file.changes.map((change) => (
            <li key={change}>Adjusted: {change}</li>
          ))}
        </ul>
      )}
      {file.flags.length > 0 && (
        <ul className="flags" aria-label="Couldn't be adjusted">
          {file.flags.map((flag, i) => (
            <li key={i}>
              <span className="flag-mark" aria-hidden="true">!</span>
              <span>
                {flag.line > 0 && <span className="muted">Line {flag.line}: </span>}
                <Message text={flag.message} />
              </span>
            </li>
          ))}
        </ul>
      )}
      <DiffView diff={file.diff} />
    </li>
  );
}

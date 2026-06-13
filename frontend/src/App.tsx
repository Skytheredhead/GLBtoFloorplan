import {
  AlertCircle,
  BarChart3,
  Box,
  CheckCircle2,
  Download,
  FileArchive,
  FileText,
  Folder,
  HelpCircle,
  Home,
  LoaderCircle,
  LogOut,
  Menu,
  MoreHorizontal,
  Upload,
  User,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  API_BASE,
  ApiError,
  getFloorplan,
  getMe,
  listFloorplans,
  loginWithGoogle,
  uploadFloorplan,
  withToken,
} from "./api";
import type {
  FloorplanDetail,
  FloorplanSummary,
  JobSnapshot,
  MeResponse,
  PublicUser,
  Quota,
} from "./types";

const GOOGLE_CLIENT_ID = import.meta.env.VITE_GOOGLE_CLIENT_ID || "";
const TOKEN_KEY = "glb-floorplan-token";

function App() {
  const [token, setToken] = useState<string | null>(() =>
    localStorage.getItem(TOKEN_KEY),
  );
  const [user, setUser] = useState<PublicUser | null>(null);
  const [quota, setQuota] = useState<Quota | null>(null);
  const [floorplans, setFloorplans] = useState<FloorplanSummary[]>([]);
  const [active, setActive] = useState<FloorplanSummary | null>(null);
  const [job, setJob] = useState<JobSnapshot | null>(null);
  const [isDragging, setDragging] = useState(false);
  const [isUploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const refreshAccount = useCallback(
    async (authToken = token) => {
      if (!authToken) return;
      const [me, saved] = await Promise.all([
        getMe(authToken),
        listFloorplans(authToken),
      ]);
      setUser(me.user);
      setQuota(me.quota);
      setFloorplans(saved);
      setActive((current) => current ?? saved[0] ?? null);
    },
    [token],
  );

  useEffect(() => {
    if (!token) return;
    refreshAccount(token).catch((err) => {
      console.error(err);
      localStorage.removeItem(TOKEN_KEY);
      setToken(null);
    });
  }, [refreshAccount, token]);

  useEffect(() => {
    if (!token || !active || active.status === "complete" || active.status === "failed") {
      return;
    }

    const events = new EventSource(
      `${API_BASE}/api/floorplans/${active.id}/events?token=${encodeURIComponent(
        token,
      )}`,
    );

    events.addEventListener("progress", (event) => {
      const snapshot = JSON.parse((event as MessageEvent).data) as JobSnapshot;
      setJob(snapshot);
      if (snapshot.status === "complete" || snapshot.status === "failed") {
        events.close();
        getFloorplan(token, active.id)
          .then((detail: FloorplanDetail) => {
            setActive(detail.floorplan);
            setJob(detail.job ?? snapshot);
            return refreshAccount(token);
          })
          .catch((err) => setError(messageForError(err)));
      }
    });

    events.onerror = () => {
      events.close();
      pollFloorplan(token, active.id, setActive, setJob, refreshAccount).catch(
        (err) => setError(messageForError(err)),
      );
    };

    return () => events.close();
  }, [active, refreshAccount, token]);

  const signIn = async (idToken: string) => {
    setError(null);
    const response = await loginWithGoogle(idToken);
    localStorage.setItem(TOKEN_KEY, response.token);
    setToken(response.token);
    setUser(response.user);
    setQuota(response.quota);
    await refreshAccount(response.token);
  };

  const signOut = () => {
    localStorage.removeItem(TOKEN_KEY);
    setToken(null);
    setUser(null);
    setQuota(null);
    setFloorplans([]);
    setActive(null);
    setJob(null);
  };

  const handleFile = async (file: File) => {
    if (!token || isUploading) return;
    setError(null);
    setUploading(true);
    try {
      const response = await uploadFloorplan(token, file);
      setActive(response.floorplan);
      setJob(response.job);
      setFloorplans((items) => [response.floorplan, ...items]);
    } catch (err) {
      setError(messageForError(err));
    } finally {
      setUploading(false);
    }
  };

  const currentStep = useMemo(() => {
    if (active?.status === "complete") return 3;
    if (active && active.status !== "failed") return 2;
    return 1;
  }, [active]);

  if (!token || !user || !quota) {
    return <SignInScreen onSignIn={signIn} error={error} setError={setError} />;
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Main">
        <button className="nav-item active" type="button">
          <Home size={19} />
          <span>New Floorplan</span>
        </button>
        <button className="nav-item" type="button">
          <Folder size={19} />
          <span>My Floorplans</span>
        </button>
        <button className="nav-item" type="button">
          <User size={19} />
          <span>Account</span>
        </button>
        <button className="nav-item" type="button">
          <BarChart3 size={19} />
          <span>Usage</span>
        </button>
        <button className="nav-item signout" type="button" onClick={signOut}>
          <LogOut size={19} />
          <span>Sign out</span>
        </button>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="brand">
            <Menu className="mobile-menu" size={21} />
            <Home size={19} />
            <strong>GLB to Floorplan</strong>
          </div>
          <div className="topbar-actions">
            <span className="quota">
              <b>{quota.remaining}</b> / {quota.monthly_limit} free saves this
              month
            </span>
            <button className="icon-button" type="button" aria-label="Help">
              <HelpCircle size={19} />
            </button>
            <button className="avatar" type="button" title={user.email}>
              {(user.name || user.email).slice(0, 1).toUpperCase()}
            </button>
          </div>
        </header>

        {error && (
          <div className="error-banner">
            <AlertCircle size={18} />
            <span>{error}</span>
            <button type="button" onClick={() => setError(null)}>
              <X size={16} />
            </button>
          </div>
        )}

        <section className="flow-grid">
          <WorkflowRail
            currentStep={currentStep}
            active={active}
            job={job}
            floorplans={floorplans}
            onPick={setActive}
            token={token}
          />

          <section className="canvas-region">
            <UploadDropZone
              hidden={currentStep !== 1}
              isDragging={isDragging}
              isUploading={isUploading}
              fileInputRef={fileInputRef}
              onFile={handleFile}
              setDragging={setDragging}
            />

            {currentStep === 2 && (
              <ProcessingView
                job={job}
                title={active?.source_filename || active?.title || "model.glb"}
              />
            )}

            {currentStep === 3 && active && (
              <ResultView floorplan={active} token={token} />
            )}

            {active?.status === "failed" && (
              <FailedView
                message={
                  active.failure_reason ||
                  job?.error ||
                  "The model could not be converted."
                }
              />
            )}
          </section>
        </section>
      </main>
    </div>
  );
}

function SignInScreen({
  onSignIn,
  error,
  setError,
}: {
  onSignIn: (idToken: string) => Promise<void>;
  error: string | null;
  setError: (value: string | null) => void;
}) {
  const googleButtonRef = useRef<HTMLDivElement>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!GOOGLE_CLIENT_ID || !googleButtonRef.current) return;

    const render = () => {
      if (!window.google || !googleButtonRef.current) return;
      window.google.accounts.id.initialize({
        client_id: GOOGLE_CLIENT_ID,
        callback: (response) => {
          setBusy(true);
          onSignIn(response.credential)
            .catch((err) => setError(messageForError(err)))
            .finally(() => setBusy(false));
        },
      });
      window.google.accounts.id.renderButton(googleButtonRef.current, {
        theme: "outline",
        size: "large",
        text: "continue_with",
        width: 280,
      });
    };

    if (window.google) {
      render();
      return;
    }

    const script = document.createElement("script");
    script.src = "https://accounts.google.com/gsi/client";
    script.async = true;
    script.defer = true;
    script.onload = render;
    document.head.appendChild(script);
  }, [onSignIn, setError]);

  const demoSignIn = async () => {
    setBusy(true);
    setError(null);
    try {
      await onSignIn("dev:skylar@example.com");
    } catch (err) {
      setError(messageForError(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="signin">
      <div className="signin-panel">
        <div className="signin-mark">
          <Box size={28} />
        </div>
        <h1>GLB to Floorplan</h1>
        <p>Import a 3D scan and export a measured floorplan PDF.</p>
        {GOOGLE_CLIENT_ID ? (
          <div className="google-button" ref={googleButtonRef} />
        ) : (
          <button className="primary wide" type="button" onClick={demoSignIn}>
            {busy ? <LoaderCircle className="spin" size={18} /> : <User size={18} />}
            Continue as demo
          </button>
        )}
        {error && (
          <div className="signin-error">
            <AlertCircle size={17} />
            <span>{error}</span>
          </div>
        )}
      </div>
    </main>
  );
}

function WorkflowRail({
  currentStep,
  active,
  job,
  floorplans,
  onPick,
  token,
}: {
  currentStep: number;
  active: FloorplanSummary | null;
  job: JobSnapshot | null;
  floorplans: FloorplanSummary[];
  onPick: (floorplan: FloorplanSummary) => void;
  token: string;
}) {
  return (
    <aside className="workflow-rail">
      <StepHeader step={1} active={currentStep === 1} title="Upload your GLB file">
        <p>
          We'll analyze your 3D model and convert it into a measured floorplan
          with rooms, doors, windows, and furniture detected.
        </p>
        <span className="support-note">Supports .glb / .gltf files up to 250MB</span>
      </StepHeader>

      <StepHeader step={2} active={currentStep === 2} title="Processing your model">
        <ProgressList job={job} />
      </StepHeader>

      <StepHeader step={3} active={currentStep === 3} title="Your floorplan is ready">
        <p>Review, then download or share your measured floorplan.</p>
        {active?.status === "complete" && (
          <a
            className="primary export"
            href={withToken(active.pdf_url, token)}
            download
          >
            <Download size={18} />
            Share / Export PDF
          </a>
        )}
        <SavedList items={floorplans} onPick={onPick} />
      </StepHeader>
    </aside>
  );
}

function StepHeader({
  step,
  active,
  title,
  children,
}: {
  step: number;
  active: boolean;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className={`step-block ${active ? "active" : ""}`}>
      <div className="step-title">
        <span>{step}</span>
        <h2>{title}</h2>
      </div>
      <div className="step-content">{children}</div>
    </section>
  );
}

function UploadDropZone({
  hidden,
  isDragging,
  isUploading,
  fileInputRef,
  onFile,
  setDragging,
}: {
  hidden: boolean;
  isDragging: boolean;
  isUploading: boolean;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  onFile: (file: File) => void;
  setDragging: (value: boolean) => void;
}) {
  if (hidden) return null;

  return (
    <div className="empty-canvas">
      <div
        className={`drop-zone ${isDragging ? "dragging" : ""}`}
        onDragOver={(event) => {
          event.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          const file = event.dataTransfer.files[0];
          if (file) onFile(file);
        }}
      >
        <FileArchive size={54} />
        <strong>No file selected</strong>
        <span>Drag & drop your GLB file here</span>
        <button
          className="primary"
          type="button"
          onClick={() => fileInputRef.current?.click()}
          disabled={isUploading}
        >
          {isUploading ? (
            <LoaderCircle className="spin" size={18} />
          ) : (
            <Upload size={18} />
          )}
          Choose file
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept=".glb,.gltf,model/gltf-binary,model/gltf+json"
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file) onFile(file);
            event.currentTarget.value = "";
          }}
        />
      </div>
    </div>
  );
}

function ProcessingView({ job, title }: { job: JobSnapshot | null; title: string }) {
  const progress = job?.progress ?? 0;
  return (
    <div className="processing-view">
      <div className="progress-ring" style={{ "--progress": progress } as React.CSSProperties}>
        <span>{progress}%</span>
      </div>
      <h2>{job?.step || "Processing your model..."}</h2>
      <p>{title}</p>
      <span className="processing-note">This usually takes 20-60 seconds.</span>
    </div>
  );
}

function ResultView({
  floorplan,
  token,
}: {
  floorplan: FloorplanSummary;
  token: string;
}) {
  const svgSrc = withToken(floorplan.svg_url, token);
  return (
    <div className="result-view">
      <div className="sheet-toolbar">
        <div>
          <strong>{floorplan.title}</strong>
          <span>
            {floorplan.total_area_sqft
              ? `${Math.round(floorplan.total_area_sqft)} sq ft`
              : "Measured floorplan"}
          </span>
        </div>
        <a className="primary" href={withToken(floorplan.pdf_url, token)} download>
          <Download size={18} />
          Share / Export PDF
        </a>
      </div>
      <div className="paper-sheet">
        {svgSrc ? (
          <img src={svgSrc} alt={`${floorplan.title} measured floorplan`} />
        ) : (
          <LoaderCircle className="spin" size={28} />
        )}
      </div>
      <div className="zoom-tools" aria-label="Preview controls">
        <button type="button">+</button>
        <button type="button">-</button>
        <button type="button">⌖</button>
      </div>
    </div>
  );
}

function FailedView({ message }: { message: string }) {
  return (
    <div className="failed-view">
      <AlertCircle size={42} />
      <h2>Processing failed</h2>
      <p>{message}</p>
    </div>
  );
}

function ProgressList({ job }: { job: JobSnapshot | null }) {
  const progress = job?.progress ?? 0;
  const steps = [
    ["Analyzing 3D geometry", 8],
    ["Detecting rooms & openings", 38],
    ["Identifying furniture", 54],
    ["Calculating measurements", 64],
    ["Generating floorplan", 82],
  ] as const;

  return (
    <ul className="progress-list">
      {steps.map(([label, threshold]) => {
        const done = progress > threshold;
        const current = job?.step === label || (!done && progress >= threshold - 16);
        return (
          <li key={label} className={done ? "done" : current ? "current" : ""}>
            {done ? <CheckCircle2 size={15} /> : <LoaderCircle size={15} />}
            <span>{label}</span>
          </li>
        );
      })}
    </ul>
  );
}

function SavedList({
  items,
  onPick,
}: {
  items: FloorplanSummary[];
  onPick: (floorplan: FloorplanSummary) => void;
}) {
  const saved = items.slice(0, 5);
  if (saved.length === 0) {
    return (
      <div className="saved-empty">
        <FileText size={18} />
        <span>Your saved floorplans will appear here.</span>
      </div>
    );
  }

  return (
    <div className="saved-list">
      {saved.map((item) => (
        <button key={item.id} type="button" onClick={() => onPick(item)}>
          <FileText size={19} />
          <span>
            <strong>{item.title}</strong>
            <small>{item.status === "complete" ? "PDF ready" : item.status}</small>
          </span>
          <MoreHorizontal size={18} />
        </button>
      ))}
    </div>
  );
}

async function pollFloorplan(
  token: string,
  id: string,
  setActive: (floorplan: FloorplanSummary) => void,
  setJob: (job: JobSnapshot | null) => void,
  refreshAccount: (token: string) => Promise<void>,
) {
  for (let attempt = 0; attempt < 90; attempt += 1) {
    const detail = await getFloorplan(token, id);
    setActive(detail.floorplan);
    setJob(detail.job ?? null);
    if (detail.floorplan.status === "complete" || detail.floorplan.status === "failed") {
      await refreshAccount(token);
      return;
    }
    await new Promise((resolve) => window.setTimeout(resolve, 1000));
  }
}

function messageForError(err: unknown) {
  if (err instanceof ApiError) return err.message;
  if (err instanceof Error) return err.message;
  return "Something went wrong.";
}

export default App;

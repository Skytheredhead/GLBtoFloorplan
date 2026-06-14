import {
  AlertCircle,
  Box,
  CheckCircle2,
  Download,
  FileArchive,
  FileText,
  HelpCircle,
  Home,
  LoaderCircle,
  MoreHorizontal,
  Upload,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  API_BASE,
  ApiError,
  apiUrl,
  getFloorplan,
  getQuota,
  uploadFloorplan,
} from "./api";
import type {
  FloorplanDetail,
  FloorplanSummary,
  JobSnapshot,
  Quota,
} from "./types";

function App() {
  const [quota, setQuota] = useState<Quota | null>(null);
  const [recentFloorplans, setRecentFloorplans] = useState<FloorplanSummary[]>([]);
  const [active, setActive] = useState<FloorplanSummary | null>(null);
  const [job, setJob] = useState<JobSnapshot | null>(null);
  const [isDragging, setDragging] = useState(false);
  const [isUploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const refreshQuota = useCallback(async () => {
    setQuota(await getQuota());
  }, []);

  useEffect(() => {
    refreshQuota().catch((err) => setError(messageForError(err)));
  }, [refreshQuota]);

  useEffect(() => {
    if (!active || active.status === "complete" || active.status === "failed") {
      return;
    }

    const events = new EventSource(`${API_BASE}/api/floorplans/${active.id}/events`);

    events.addEventListener("progress", (event) => {
      const snapshot = JSON.parse((event as MessageEvent).data) as JobSnapshot;
      setJob(snapshot);
      if (snapshot.status === "complete" || snapshot.status === "failed") {
        events.close();
        getFloorplan(active.id)
          .then((detail: FloorplanDetail) => {
            setActive(detail.floorplan);
            setJob(detail.job ?? snapshot);
            setRecentFloorplans((items) =>
              [detail.floorplan, ...items.filter((item) => item.id !== detail.floorplan.id)].slice(
                0,
                5,
              ),
            );
          })
          .catch((err) => setError(messageForError(err)));
      }
    });

    events.onerror = () => {
      events.close();
      pollFloorplan(active.id, setActive, setJob).catch((err) =>
        setError(messageForError(err)),
      );
    };

    return () => events.close();
  }, [active]);

  const handleFile = async (file: File) => {
    if (isUploading) return;
    setError(null);
    setUploading(true);
    try {
      const response = await uploadFloorplan(file);
      setQuota(response.quota);
      setActive(response.floorplan);
      setJob(response.job);
      setRecentFloorplans((items) => [response.floorplan, ...items].slice(0, 5));
    } catch (err) {
      setError(messageForError(err));
      refreshQuota().catch(() => undefined);
    } finally {
      setUploading(false);
    }
  };

  const currentStep = useMemo(() => {
    if (active?.status === "complete") return 3;
    if (active && active.status !== "failed") return 2;
    return 1;
  }, [active]);

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Main">
        <button className="nav-item active" type="button">
          <Home size={19} />
          <span>New Floorplan</span>
        </button>
        <button className="nav-item" type="button">
          <FileText size={19} />
          <span>Recent</span>
        </button>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="brand">
            <Box size={19} />
            <strong>GLB to Floorplan</strong>
          </div>
          <div className="topbar-actions">
            <span className="quota">
              <b>{quota?.remaining ?? "-"}</b> / {quota?.daily_limit ?? 5} converts
              left today
            </span>
            <button className="icon-button" type="button" aria-label="Help">
              <HelpCircle size={19} />
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
            floorplans={recentFloorplans}
            onPick={setActive}
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

            {currentStep === 3 && active && <ResultView floorplan={active} />}

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

function WorkflowRail({
  currentStep,
  active,
  job,
  floorplans,
  onPick,
}: {
  currentStep: number;
  active: FloorplanSummary | null;
  job: JobSnapshot | null;
  floorplans: FloorplanSummary[];
  onPick: (floorplan: FloorplanSummary) => void;
}) {
  return (
    <aside className="workflow-rail">
      <StepHeader step={1} active={currentStep === 1} title="Upload your model">
        <p>
          We'll analyze your 3D model and convert it into a measured floorplan
          with rooms, doors, windows, and furniture detected.
        </p>
        <span className="support-note">Supports .glb, .gltf, or .zip files up to 250MB</span>
      </StepHeader>

      <StepHeader step={2} active={currentStep === 2} title="Processing your model">
        <ProgressList job={job} />
      </StepHeader>

      <StepHeader step={3} active={currentStep === 3} title="Your floorplan is ready">
        <p>Review, then download your measured floorplan.</p>
        {active?.status === "complete" && (
          <a className="primary export" href={apiUrl(active.pdf_url)} download>
            <Download size={18} />
            Export PDF
          </a>
        )}
        <RecentList items={floorplans} onPick={onPick} />
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
        <span>Drag & drop your GLB, GLTF, or ZIP file here</span>
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
          accept=".glb,.gltf,.zip,model/gltf-binary,model/gltf+json,application/zip"
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

function ResultView({ floorplan }: { floorplan: FloorplanSummary }) {
  const svgSrc = apiUrl(floorplan.svg_url);
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
        <a className="primary" href={apiUrl(floorplan.pdf_url)} download>
          <Download size={18} />
          Export PDF
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

function RecentList({
  items,
  onPick,
}: {
  items: FloorplanSummary[];
  onPick: (floorplan: FloorplanSummary) => void;
}) {
  if (items.length === 0) {
    return (
      <div className="recent-empty">
        <FileText size={18} />
        <span>Recent conversions will appear here until the server restarts.</span>
      </div>
    );
  }

  return (
    <div className="recent-list">
      {items.map((item) => (
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
  id: string,
  setActive: (floorplan: FloorplanSummary) => void,
  setJob: (job: JobSnapshot | null) => void,
) {
  for (let attempt = 0; attempt < 90; attempt += 1) {
    const detail = await getFloorplan(id);
    setActive(detail.floorplan);
    setJob(detail.job ?? null);
    if (detail.floorplan.status === "complete" || detail.floorplan.status === "failed") {
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

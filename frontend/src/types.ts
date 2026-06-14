export interface Quota {
  daily_limit: number;
  used: number;
  remaining: number;
  day: string;
  reset_at: string;
}

export interface FloorplanSummary {
  id: string;
  title: string;
  status: "queued" | "processing" | "complete" | "failed" | string;
  source_filename: string;
  source_size_bytes: number;
  confidence: number;
  total_area_sqft?: number;
  width_ft?: number;
  depth_ft?: number;
  failure_reason?: string;
  created_at: string;
  svg_url?: string;
  pdf_url?: string;
}

export interface JobSnapshot {
  floorplan_id: string;
  status: string;
  progress: number;
  step: string;
  error?: string;
}

export interface UploadResponse {
  floorplan: FloorplanSummary;
  job: JobSnapshot;
  quota: Quota;
}

export interface FloorplanDetail {
  floorplan: FloorplanSummary;
  job?: JobSnapshot;
}

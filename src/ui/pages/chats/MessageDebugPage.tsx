import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";

import { getMessageDebugSnapshot, type ChatMessageDebugSnapshot } from "../../../core/chat/manager";
import { getSession, readSettings } from "../../../core/storage/repo";
import type { Session, Settings, StoredMessage } from "../../../core/storage/schemas";
import {
  getMessageDebugTrace,
  subscribeChatDebugStore,
  type ChatDebugEventRecord,
  type ChatRequestDebugTrace,
} from "../../../core/debug/chatDebugStore";

type DebugAttempt = {
  request?: ChatDebugEventRecord;
  response?: ChatDebugEventRecord;
  providerError?: ChatDebugEventRecord;
  transportRetries: ChatDebugEventRecord[];
};

const REQUEST_STATES = new Set(["sending_request", "regenerate_request", "continue_request"]);
const RESPONSE_STATES = new Set(["response", "regenerate_response", "continue_response"]);
const ERROR_STATES = new Set([
  "provider_error",
  "regenerate_provider_error",
  "continue_provider_error",
]);

function formatTimestamp(value: number) {
  return new Date(value).toLocaleString();
}

function stringify(value: unknown) {
  return JSON.stringify(value, null, 2);
}

function getPayloadObject(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object") return null;
  return value as Record<string, unknown>;
}

function extractAttempts(trace: ChatRequestDebugTrace | null): DebugAttempt[] {
  if (!trace) return [];

  const attempts: DebugAttempt[] = [];
  let current: DebugAttempt | null = null;

  for (const event of trace.events) {
    if (REQUEST_STATES.has(event.state)) {
      current = { request: event, transportRetries: [] };
      attempts.push(current);
      continue;
    }
    if (!current) continue;
    if (event.state === "transport_retry") {
      current.transportRetries.push(event);
      continue;
    }
    if (RESPONSE_STATES.has(event.state)) {
      current.response = event;
      continue;
    }
    if (ERROR_STATES.has(event.state)) {
      current.providerError = event;
    }
  }

  return attempts;
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start justify-between gap-6 border-b border-white/10 py-2 last:border-b-0">
      <div className="text-white/45">{label}</div>
      <div className="text-right text-white">{value}</div>
    </div>
  );
}

function JsonBlock({ title, value }: { title: string; value: unknown }) {
  return (
    <section className="space-y-2">
      <h2 className="text-sm font-semibold text-white">{title}</h2>
      <pre className="overflow-x-auto rounded border border-white/10 bg-black px-3 py-3 text-xs leading-6 text-white/85">
        {stringify(value)}
      </pre>
    </section>
  );
}

export function MessageDebugPage() {
  const navigate = useNavigate();
  const { sessionId, messageId } = useParams<{
    characterId: string;
    sessionId: string;
    messageId: string;
  }>();
  const [session, setSession] = useState<Session | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [snapshot, setSnapshot] = useState<ChatMessageDebugSnapshot | null>(null);
  const [snapshotError, setSnapshotError] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionId) return;
    let cancelled = false;

    void getSession(sessionId)
      .then((next) => {
        if (!cancelled) setSession(next);
      })
      .catch((error) => {
        console.error("Failed to load session for debug page:", error);
        if (!cancelled) setSession(null);
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  useEffect(() => {
    void readSettings()
      .then(setSettings)
      .catch((error) => console.error("Failed to load settings for debug page:", error));
  }, []);

  useEffect(() => {
    if (!sessionId || !messageId) return;
    let cancelled = false;

    setSnapshot(null);
    setSnapshotError(null);
    void getMessageDebugSnapshot({ sessionId, messageId })
      .then((next) => {
        if (!cancelled) setSnapshot(next);
      })
      .catch((error) => {
        console.error("Failed to reconstruct message debug snapshot:", error);
        if (!cancelled) {
          setSnapshotError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [messageId, sessionId]);

  const trace = useSyncExternalStore(
    subscribeChatDebugStore,
    () => (sessionId && messageId ? getMessageDebugTrace(sessionId, messageId) : null),
    () => null,
  );

  const message = useMemo<StoredMessage | null>(() => {
    if (!session || !messageId) return null;
    return session.messages.find((item) => item.id === messageId) ?? null;
  }, [messageId, session]);

  const attempts = useMemo(() => extractAttempts(trace), [trace]);
  const latestAttempt = attempts[attempts.length - 1];
  const totalTransportRetries = attempts.reduce(
    (sum, attempt) => sum + attempt.transportRetries.length,
    0,
  );
  const latestRequestPayload = getPayloadObject(latestAttempt?.request?.payload);
  const latestResponsePayload = getPayloadObject(latestAttempt?.response?.payload);
  const resolvedModel = useMemo(() => {
    if (!settings || !message?.modelId) return null;
    return settings.models.find((item) => item.id === message.modelId) ?? null;
  }, [message?.modelId, settings]);
  const providerLabel = String(
    latestRequestPayload?.providerId ??
      snapshot?.providerId ??
      resolvedModel?.providerId ??
      "unknown",
  );
  const modelLabel = String(
    latestRequestPayload?.model ??
      snapshot?.modelDisplayName ??
      resolvedModel?.displayName ??
      "unknown",
  );
  const inferredOperation =
    trace?.operation ??
    snapshot?.operation ??
    (message?.variants && message.variants.length > 1 ? "regenerate_or_variant" : "completion");
  const requestBody = latestRequestPayload?.requestBody ?? snapshot?.requestBody;
  const requestBodyObject = getPayloadObject(requestBody);
  const requestMessages = requestBodyObject?.messages ?? snapshot?.requestMessages;
  const requestSettings = latestRequestPayload?.requestSettings ?? snapshot?.requestSettings;
  const promptEntries = snapshot?.promptEntries ?? null;
  const relativePromptEntries = snapshot?.relativePromptEntries ?? null;
  const inChatPromptEntries = snapshot?.inChatPromptEntries ?? null;

  return (
    <div className="h-full overflow-y-auto bg-[#050505] px-4 py-4 text-sm text-white">
      <div className="mx-auto max-w-5xl space-y-6">
        <button
          type="button"
          onClick={() => navigate(-1)}
          className="inline-flex items-center gap-2 rounded border border-white/10 bg-[#0d0d0d] px-3 py-2 font-mono text-xs text-white/80 hover:bg-[#141414]"
        >
          <ArrowLeft className="h-4 w-4" />
          Return
        </button>

        <section className="rounded border border-white/10 bg-[#0d0d0d] p-4 font-mono text-xs">
          <div className="mb-3 text-sm font-semibold text-white">Message Debug</div>
          <SummaryRow label="Session ID" value={sessionId ?? "unknown"} />
          <SummaryRow label="Message ID" value={messageId ?? "unknown"} />
          <SummaryRow label="Role" value={message?.role ?? "unknown"} />
          <SummaryRow label="Request ID" value={trace?.requestId ?? "missing"} />
          <SummaryRow label="Operation" value={inferredOperation} />
          <SummaryRow label="Provider" value={providerLabel} />
          <SummaryRow label="Model" value={modelLabel} />
          <SummaryRow label="Attempt count" value={String(attempts.length)} />
          <SummaryRow label="Transport retries" value={String(totalTransportRetries)} />
          <SummaryRow
            label="Request time"
            value={
              latestResponsePayload?.elapsedMs != null
                ? `${String(latestResponsePayload.elapsedMs)} ms`
                : "unknown"
            }
          />
          <SummaryRow
            label="Tokens"
            value={String(message?.usage?.totalTokens ?? message?.usage?.completionTokens ?? 0)}
          />
          <SummaryRow
            label="Prompt / completion"
            value={`${message?.usage?.promptTokens ?? 0} / ${message?.usage?.completionTokens ?? 0}`}
          />
          <SummaryRow
            label="Created"
            value={message ? formatTimestamp(message.createdAt) : "unknown"}
          />
        </section>

        {!trace ? (
          <section className="rounded border border-white/10 bg-[#0d0d0d] p-4 font-mono text-xs text-white/70">
            No in-memory debug trace found for this message. Showing a reconstructed request
            snapshot from the current session state where possible.
          </section>
        ) : null}

        {snapshot?.notes?.length ? (
          <section className="space-y-2 rounded border border-white/10 bg-[#0d0d0d] p-4 font-mono text-xs text-white/80">
            <div className="text-sm font-semibold text-white">Reconstruction Notes</div>
            {snapshot.notes.map((note, index) => (
              <div key={`${index}-${note}`} className="text-white/70">
                {note}
              </div>
            ))}
          </section>
        ) : null}

        {snapshotError ? (
          <section className="rounded border border-red-500/20 bg-red-500/5 p-4 font-mono text-xs text-red-200">
            Failed to reconstruct request snapshot: {snapshotError}
          </section>
        ) : null}

        {attempts.map((attempt, index) => (
          <section
            key={`${trace?.requestId ?? "trace"}-${index}`}
            className="space-y-3 rounded border border-white/10 bg-[#0d0d0d] p-4 font-mono text-xs"
          >
            <div className="text-sm font-semibold text-white">Attempt {index + 1}</div>
            {attempt.transportRetries.length > 0 ? (
              <div className="rounded border border-yellow-500/20 bg-yellow-500/5 px-3 py-2 text-yellow-200">
                {attempt.transportRetries.length} transport retr
                {attempt.transportRetries.length === 1 ? "y" : "ies"} before final provider
                response.
              </div>
            ) : null}
            {attempt.request ? <JsonBlock title="Request" value={attempt.request.payload} /> : null}
            {attempt.response ? (
              <JsonBlock title="Response" value={attempt.response.payload} />
            ) : null}
            {attempt.providerError ? (
              <JsonBlock title="Provider Error" value={attempt.providerError.payload} />
            ) : null}
            {attempt.transportRetries.length > 0 ? (
              <JsonBlock
                title="Transport Retries"
                value={attempt.transportRetries.map((event) => event.payload)}
              />
            ) : null}
          </section>
        ))}

        {requestSettings ? <JsonBlock title="Request Settings" value={requestSettings} /> : null}
        {promptEntries ? <JsonBlock title="System Prompt Entries" value={promptEntries} /> : null}
        {relativePromptEntries ? (
          <JsonBlock title="Relative Prompt Entries" value={relativePromptEntries} />
        ) : null}
        {inChatPromptEntries ? (
          <JsonBlock title="In-Chat Prompt Entries" value={inChatPromptEntries} />
        ) : null}
        {requestMessages ? (
          <JsonBlock title="Resolved Request Messages" value={requestMessages} />
        ) : null}
        {requestBody !== undefined ? (
          <JsonBlock title="Full Request Body" value={requestBody} />
        ) : null}
        <JsonBlock title="Full Session JSON" value={session} />
        <JsonBlock title="Stored Message JSON" value={message} />
        <JsonBlock title="Full Trace Events" value={trace?.events ?? []} />
      </div>
    </div>
  );
}

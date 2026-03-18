import { useState, type ReactElement } from "react";

interface JsonToggleProps {
  data: unknown;
  label?: string;
}

export function JsonToggle({ data, label }: JsonToggleProps): ReactElement {
  const [viewMode, setViewMode] = useState<"json" | "rendered">("rendered");

  const dataObj = data as Record<string, unknown>;

  return (
    <div className="json-toggle-container">
      <div className="detail-header">
        {label && <span className="json-toggle-label">{label}</span>}
        <div className="view-toggle">
          <button
            className={viewMode === "rendered" ? "active" : ""}
            onClick={() => setViewMode("rendered")}
          >
            Rendered
          </button>
          <button
            className={viewMode === "json" ? "active" : ""}
            onClick={() => setViewMode("json")}
          >
            JSON
          </button>
        </div>
      </div>
      {viewMode === "json" ? (
        <pre className="json-raw">{JSON.stringify(data, null, 2)}</pre>
      ) : (
        <RenderedView data={dataObj} />
      )}
    </div>
  );
}

function RenderedView({ data }: { data: Record<string, unknown> }): ReactElement {
  if (!data || typeof data !== "object") {
    return <div className="rendered-empty">No data</div>;
  }

  const entries = Object.entries(data);

  return (
    <div className="rendered-view">
      {entries.map(([key, value]) => (
        <RenderedField key={key} name={key} value={value} />
      ))}
    </div>
  );
}

function RenderedField({ name, value }: { name: string; value: unknown }): ReactElement {
  if (value === null || value === undefined) {
    return (
      <div className="rendered-field">
        <label className="rendered-label">{name}</label>
        <span className="rendered-null">null</span>
      </div>
    );
  }

  if (typeof value === "boolean") {
    return (
      <div className="rendered-field">
        <label className="rendered-label">{name}</label>
        <span className={value ? "rendered-boolean-true" : "rendered-boolean-false"}>
          {value ? "true" : "false"}
        </span>
      </div>
    );
  }

  if (typeof value === "number") {
    return (
      <div className="rendered-field">
        <label className="rendered-label">{name}</label>
        <span className="rendered-number">{value}</span>
      </div>
    );
  }

  if (typeof value === "string") {
    if (name === "statement" || name === "description" || name === "content" || name === "system" || name === "error" || name === "message") {
      return (
        <div className="rendered-field">
          <label className="rendered-label">{name}</label>
          <div className="rendered-string-block">{value}</div>
        </div>
      );
    }
    return (
      <div className="rendered-field">
        <label className="rendered-label">{name}</label>
        <span className="rendered-string">{value}</span>
      </div>
    );
  }

  if (Array.isArray(value)) {
    if (value.length === 0) {
      return (
        <div className="rendered-field">
          <label className="rendered-label">{name}</label>
          <span className="rendered-empty">(empty array)</span>
        </div>
      );
    }

    const firstItem = value[0];
    if (typeof firstItem === "object" && firstItem !== null) {
      return (
        <div className="rendered-field">
          <label className="rendered-label">{name} ({value.length} items)</label>
          <div className="rendered-array">
            {value.map((item, idx) => (
              <div key={idx} className="rendered-array-item">
                <RenderedField name={`[${idx}]`} value={item} />
              </div>
            ))}
          </div>
        </div>
      );
    }

    return (
      <div className="rendered-field">
        <label className="rendered-label">{name} ({value.length})</label>
        <div className="rendered-string-array">
          {value.map((item, idx) => (
            <span key={idx} className="rendered-array-tag">{String(item)}</span>
          ))}
        </div>
      </div>
    );
  }

  if (typeof value === "object") {
    const objEntries = Object.entries(value as Record<string, unknown>);
    if (objEntries.length === 0) {
      return (
        <div className="rendered-field">
          <label className="rendered-label">{name}</label>
          <span className="rendered-empty">(empty object)</span>
        </div>
      );
    }

    return (
      <div className="rendered-field">
        <label className="rendered-label">{name}</label>
        <div className="rendered-nested">
          {objEntries.map(([key, val]) => (
            <RenderedField key={key} name={key} value={val} />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="rendered-field">
      <label className="rendered-label">{name}</label>
      <span>{String(value)}</span>
    </div>
  );
}

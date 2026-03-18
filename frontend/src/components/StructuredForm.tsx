import { useState, type ReactElement } from "react";

interface Field {
  name: string;
  type: "string" | "number" | "boolean" | "object" | "array" | "array-string";
  label?: string;
  placeholder?: string;
  required?: boolean;
}

interface Section {
  name: string;
  label: string;
  fields: Field[];
}

interface StructuredFormProps {
  sections: Section[];
  value: Record<string, unknown>;
  onChange: (value: Record<string, unknown>) => void;
}

export function StructuredForm({ sections, value, onChange }: StructuredFormProps): ReactElement {
  const [viewMode, setViewMode] = useState<"form" | "json">("form");

  function handleFieldChange(sectionName: string, fieldName: string, fieldValue: unknown) {
    const sectionKey = sectionName.toLowerCase().replace(/\s+/g, "_");
    const currentSection = (value[sectionKey] as Record<string, unknown>) || {};
    const newSection = { ...currentSection, [fieldName]: fieldValue };
    onChange({ ...value, [sectionKey]: newSection });
  }

  function handleTopLevelChange(fieldName: string, fieldValue: unknown) {
    onChange({ ...value, [fieldName]: fieldValue });
  }

  return (
    <div className="structured-form">
      <div className="form-toggle">
        <button
          className={viewMode === "form" ? "active" : ""}
          onClick={() => setViewMode("form")}
        >
          Form
        </button>
        <button
          className={viewMode === "json" ? "active" : ""}
          onClick={() => setViewMode("json")}
        >
          JSON
        </button>
      </div>

      {viewMode === "json" ? (
        <textarea
          className="form-json-input"
          value={JSON.stringify(value, null, 2)}
          onChange={(e) => {
            try {
              onChange(JSON.parse(e.target.value));
            } catch {
              // Ignore invalid JSON while typing
            }
          }}
        />
      ) : (
        <div className="form-fields">
          {sections.map((section) => (
            <div key={section.name} className="form-section">
              <div className="form-section-title">{section.label}</div>
              {section.fields.map((field) => (
                <FormField
                  key={field.name}
                  field={field}
                  value={section.name === "_top" 
                    ? value[field.name] 
                    : (value[section.name.toLowerCase().replace(/\s+/g, "_")] as Record<string, unknown>)?.[field.name]
                  }
                  onChange={(val) => {
                    if (section.name === "_top") {
                      handleTopLevelChange(field.name, val);
                    } else {
                      handleFieldChange(section.name, field.name, val);
                    }
                  }}
                />
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function FormField({ field, value, onChange }: { field: Field; value: unknown; onChange: (val: unknown) => void }): ReactElement {
  if (field.type === "boolean") {
    return (
      <div className="form-field">
        <label className="form-checkbox">
          <input
            type="checkbox"
            checked={Boolean(value)}
            onChange={(e) => onChange(e.target.checked)}
          />
          <span>{field.label || field.name}</span>
        </label>
      </div>
    );
  }

  if (field.type === "array-string") {
    const arr = Array.isArray(value) ? value : [];
    const [inputValue, setInputValue] = useState("");

    function addItem() {
      if (inputValue.trim()) {
        onChange([...arr, inputValue.trim()]);
        setInputValue("");
      }
    }

    return (
      <div className="form-field">
        <label>{field.label || field.name}</label>
        <div className="form-array-input">
          <input
            type="text"
            value={inputValue}
            placeholder={field.placeholder || "Add item..."}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addItem()}
          />
          <button type="button" onClick={addItem}>Add</button>
        </div>
        <div className="form-array-tags">
          {arr.map((item, idx) => (
            <span key={idx} className="form-array-tag">
              {String(item)}
              <button type="button" onClick={() => onChange(arr.filter((_, i) => i !== idx))}>×</button>
            </span>
          ))}
        </div>
      </div>
    );
  }

  if (field.type === "object") {
    return (
      <div className="form-field">
        <label>{field.label || field.name} (JSON)</label>
        <textarea
          className="form-textarea-small"
          value={value ? JSON.stringify(value, null, 2) : ""}
          placeholder={field.placeholder || "{}"}
          onChange={(e) => {
            try {
              onChange(JSON.parse(e.target.value || "{}"));
            } catch {
              // Ignore
            }
          }}
        />
      </div>
    );
  }

  return (
    <div className="form-field">
      <label>
        {field.label || field.name}
        {field.required && <span className="required">*</span>}
      </label>
      <input
        type={field.type === "number" ? "number" : "text"}
        value={value as string || ""}
        placeholder={field.placeholder}
        onChange={(e) => onChange(field.type === "number" ? Number(e.target.value) : e.target.value)}
      />
    </div>
  );
}

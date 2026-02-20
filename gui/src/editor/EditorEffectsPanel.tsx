import type { EditorOperations } from "./editorTypes";

interface EditorEffectsPanelProps {
  ops: EditorOperations;
  onChange: (partial: Partial<EditorOperations>) => void;
}

interface SliderRowProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  format: (v: number) => string;
  onChange: (v: number) => void;
  defaultValue?: number;
}

function SliderRow({
  label,
  value,
  min,
  max,
  step,
  format,
  onChange,
  defaultValue = 0,
}: SliderRowProps) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className="editor-fx-row">
      <label className="editor-fx-label">{label}</label>
      <input
        type="range"
        className="editor-fx-slider"
        min={min}
        max={max}
        step={step}
        value={value}
        style={{ "--progress": `${pct}%` } as React.CSSProperties}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      <span className="editor-fx-value">{format(value)}</span>
      {value !== defaultValue && (
        <button
          className="editor-fx-reset"
          onClick={() => onChange(defaultValue)}
          title="Reset to default"
        >
          ↺
        </button>
      )}
    </div>
  );
}

export default function EditorEffectsPanel({ ops, onChange }: EditorEffectsPanelProps) {
  return (
    <div className="editor-effects-panel">
      <div className="editor-fx-section-title">Effects</div>

      <SliderRow
        label="Volume"
        value={ops.volume_db}
        min={-30}
        max={20}
        step={0.5}
        format={(v) => `${v >= 0 ? "+" : ""}${v.toFixed(1)} dB`}
        onChange={(v) => onChange({ volume_db: v })}
        defaultValue={0}
      />

      <SliderRow
        label="Speed"
        value={ops.speed}
        min={0.5}
        max={2.0}
        step={0.05}
        format={(v) => `${v.toFixed(2)}×`}
        onChange={(v) => onChange({ speed: v })}
        defaultValue={1.0}
      />

      <SliderRow
        label="Pitch"
        value={ops.pitch_semitones}
        min={-12}
        max={12}
        step={1}
        format={(v) => `${v >= 0 ? "+" : ""}${v} st`}
        onChange={(v) => onChange({ pitch_semitones: v })}
        defaultValue={0}
      />

      <div className="editor-fx-section-title">Fades</div>

      <SliderRow
        label="Fade In"
        value={ops.fade_in_secs}
        min={0}
        max={30}
        step={0.5}
        format={(v) => (v === 0 ? "Off" : `${v.toFixed(1)}s`)}
        onChange={(v) => onChange({ fade_in_secs: v })}
        defaultValue={0}
      />

      <SliderRow
        label="Fade Out"
        value={ops.fade_out_secs}
        min={0}
        max={30}
        step={0.5}
        format={(v) => (v === 0 ? "Off" : `${v.toFixed(1)}s`)}
        onChange={(v) => onChange({ fade_out_secs: v })}
        defaultValue={0}
      />

      <div className="editor-fx-section-title">Trim</div>

      <div className="editor-fx-row">
        <label className="editor-fx-label">In</label>
        <span className="editor-fx-value mono">
          {ops.trim_start_secs.toFixed(2)}s
        </span>
      </div>
      <div className="editor-fx-row">
        <label className="editor-fx-label">Out</label>
        <span className="editor-fx-value mono">
          {ops.trim_end_secs.toFixed(2)}s
        </span>
      </div>

      <div className="editor-fx-section-title">Processing</div>

      <div className="editor-fx-row">
        <label className="editor-fx-label">Normalize</label>
        <div className="editor-fx-toggle-wrap">
          <input
            type="checkbox"
            id="ed-normalize"
            checked={ops.normalize}
            onChange={(e) => onChange({ normalize: e.target.checked })}
          />
          <label htmlFor="ed-normalize" className="editor-fx-toggle-label">
            {ops.normalize ? "On (EBU R128)" : "Off"}
          </label>
        </div>
      </div>

      {ops.cuts.length > 0 && (
        <>
          <div className="editor-fx-section-title">
            Cuts ({ops.cuts.length})
          </div>
          {ops.cuts.map((cut, i) => (
            <div key={i} className="editor-fx-row">
              <span className="editor-fx-label mono">
                {cut.start_secs.toFixed(2)}s – {cut.end_secs.toFixed(2)}s
              </span>
            </div>
          ))}
        </>
      )}
    </div>
  );
}

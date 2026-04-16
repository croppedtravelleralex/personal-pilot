interface SearchInputProps {
  label: string;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
}

export function SearchInput({
  label,
  value,
  placeholder,
  onChange,
}: SearchInputProps) {
  return (
    <label className="field">
      <span className="field__label">{label}</span>
      <input
        className="field__input"
        type="search"
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

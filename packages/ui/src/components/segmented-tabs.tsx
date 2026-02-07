import { cn } from '../lib/cn';

export type SegmentedOption<T extends string> = {
  value: T;
  label: string;
};

export type SegmentedTabsProps<T extends string> = {
  value: T;
  onChange: (value: T) => void;
  ariaLabel: string;
  options: SegmentedOption<T>[];
  className?: string;
};

export function SegmentedTabs<T extends string>({
  value,
  onChange,
  ariaLabel,
  options,
  className,
}: SegmentedTabsProps<T>) {
  return (
    <div className={cn('ui-segmented', className)} role="tablist" aria-label={ariaLabel}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="tab"
          aria-selected={value === option.value}
          className={cn('ui-segmented__item', value === option.value && 'is-active')}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

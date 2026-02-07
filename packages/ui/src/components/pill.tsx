import type { HTMLAttributes } from 'react';
import { cn } from '../lib/cn';

export type PillProps = HTMLAttributes<HTMLSpanElement>;

export function Pill({ className, ...props }: PillProps) {
  return <span className={cn('ui-pill', className)} {...props} />;
}

import type { SVGProps } from 'react';
import { cn } from '../../lib/utils';

export type IconName =
  | 'compose'
  | 'automation'
  | 'skills'
  | 'chevronDown'
  | 'send'
  | 'mic'
  | 'branch'
  | 'settings';

export type IconProps = Omit<SVGProps<SVGSVGElement>, 'name'> & {
  name: IconName;
  size?: number;
};

function IconPath({ name }: { name: IconName }) {
  switch (name) {
    case 'compose':
      return (
        <>
          <path d="M4 20h4l10-10a2.8 2.8 0 0 0-4-4L4 16v4z" />
          <path d="m12.5 7.5 4 4" />
        </>
      );
    case 'automation':
      return (
        <>
          <path d="M12 3v5" />
          <path d="M12 16v5" />
          <path d="M4.2 7.8 8 10" />
          <path d="m16 14 3.8 2.2" />
          <path d="m4.2 16.2 3.8-2.2" />
          <path d="M16 10l3.8-2.2" />
          <circle cx="12" cy="12" r="3" />
        </>
      );
    case 'skills':
      return (
        <>
          <path d="M8 3h8l4 4v8l-4 4H8l-4-4V7l4-4z" />
          <path d="m9.5 9.5 5 5" />
          <path d="m14.5 9.5-5 5" />
        </>
      );
    case 'chevronDown':
      return <path d="m6 9 6 6 6-6" />;
    case 'send':
      return (
        <>
          <path d="M3 11.5 21 3l-8.5 18-1.8-7.7L3 11.5z" />
          <path d="M10.7 13.3 21 3" />
        </>
      );
    case 'mic':
      return (
        <>
          <rect x="9" y="3" width="6" height="11" rx="3" />
          <path d="M5 11a7 7 0 0 0 14 0" />
          <path d="M12 18v3" />
        </>
      );
    case 'branch':
      return (
        <>
          <circle cx="6" cy="6" r="2.2" />
          <circle cx="18" cy="18" r="2.2" />
          <circle cx="18" cy="6" r="2.2" />
          <path d="M8.2 6h6.6" />
          <path d="M16 8.2v5.6a4.2 4.2 0 0 0 2 3.6" />
        </>
      );
    case 'settings':
      return (
        <>
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1 1 0 0 0 .2 1.1l.1.1a1.2 1.2 0 1 1-1.7 1.7l-.1-.1a1 1 0 0 0-1.1-.2 1 1 0 0 0-.6.9V19a1.2 1.2 0 1 1-2.4 0v-.1a1 1 0 0 0-.6-.9 1 1 0 0 0-1.1.2l-.1.1a1.2 1.2 0 1 1-1.7-1.7l.1-.1a1 1 0 0 0 .2-1.1 1 1 0 0 0-.9-.6H6a1.2 1.2 0 1 1 0-2.4h.1a1 1 0 0 0 .9-.6 1 1 0 0 0-.2-1.1l-.1-.1a1.2 1.2 0 1 1 1.7-1.7l.1.1a1 1 0 0 0 1.1.2 1 1 0 0 0 .6-.9V5a1.2 1.2 0 1 1 2.4 0v.1a1 1 0 0 0 .6.9 1 1 0 0 0 1.1-.2l.1-.1a1.2 1.2 0 1 1 1.7 1.7l-.1.1a1 1 0 0 0-.2 1.1 1 1 0 0 0 .9.6H19a1.2 1.2 0 1 1 0 2.4h-.1a1 1 0 0 0-.9.6z" />
        </>
      );
    default:
      return null;
  }
}

export function Icon({ name, className, size = 16, ...props }: IconProps) {
  return (
    <svg
      className={cn('ui-icon', className)}
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={1.85}
      width={size}
      height={size}
      aria-hidden
      {...props}
    >
      <IconPath name={name} />
    </svg>
  );
}

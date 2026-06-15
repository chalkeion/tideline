import { clsx } from 'clsx'
import type { HTMLAttributes } from 'react'

export type CardProps = HTMLAttributes<HTMLDivElement>

export function Card({ className, children, ...props }: CardProps) {
  return (
    <div className={clsx('rounded-xl border p-4', className)} {...props}>
      {children}
    </div>
  )
}

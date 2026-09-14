import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
export const cn = (...i: ClassValue[]) => twMerge(clsx(i));
export const hueColor = (hue: number) => `hsl(${((hue % 360) + 360) % 360} 72% 62%)`;
export const initials = (n: string) => n.trim().split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0]!.toUpperCase()).join("") || "?";

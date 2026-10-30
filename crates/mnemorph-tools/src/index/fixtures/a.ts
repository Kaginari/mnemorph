import { readFile } from "node:fs";
import * as path from "node:path";

export const LIMIT = 10;

export function load(name: string): string {
  return name;
}

export const twice = (n: number) => n * 2;

export interface Shape {
  area(): number;
}

type Id = string;

export class Stack<T> {
  private items: T[] = [];

  push(t: T): void {
    this.items.push(t);
  }

  get size(): number {
    return this.items.length;
  }
}

enum Color {
  Red,
  Green,
}

namespace Util {
  export function noop() {}
}

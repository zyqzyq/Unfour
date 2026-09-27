import type { FlowStep } from "@unfour/command-client";

/** The engine accepts forward edges only. Intersect all reachable predecessors,
 * including implicit fallthrough; an unreachable predecessor is not a path. */
export function guaranteedUpstream(steps: FlowStep[]): Map<string, Set<string>> {
  const incoming = new Map<string, Set<string>[]>();
  if (steps[0]) incoming.set(steps[0].id, [new Set()]);
  const result = new Map<string, Set<string>>();
  steps.forEach((step, index) => {
    const paths = incoming.get(step.id);
    const safe = new Set(paths?.[0] ?? []);
    for (const path of paths ?? []) for (const id of safe) if (!path.has(id)) safe.delete(id);
    result.set(step.id, safe);
    if (!paths) return;
    const targets = step.kind === "condition" ? [step.ifTrue, step.ifFalse] : [step.next ?? steps[index + 1]?.id];
    for (const target of new Set(targets)) {
      if (!target || !steps.slice(index + 1).some((node) => node.id === target)) continue;
      incoming.set(target, [...(incoming.get(target) ?? []), new Set([...safe, step.id])]);
    }
  });
  return result;
}

type Reference = { pointer: string; invalid: boolean };
export function validPointer(pointer: string): boolean {
  return pointer.startsWith("/") && !/~(?![01])/.test(pointer) && ["inputs", "steps", "probe"].includes(pointer.split("/")[1]);
}
function referenceValues(value: unknown): Reference[] {
  if (typeof value === "string") return [...value.matchAll(/\$\{([^}]*)(\}|$)/g)].map((match) => ({ pointer: match[1], invalid: !match[2] || !validPointer(match[1]) }));
  if (!value || typeof value !== "object") return [];
  if ("$ref" in value) return [{ pointer: String(value.$ref), invalid: Object.keys(value).length !== 1 || typeof value.$ref !== "string" || !validPointer(value.$ref) }];
  return Object.values(value).flatMap(referenceValues);
}
export function referencesIn(value: unknown): string[] {
  return referenceValues(value).map((ref) => ref.pointer);
}

export function unsafeReferences(steps: FlowStep[]) {
  const upstream = guaranteedUpstream(steps);
  return steps.flatMap((step) => {
    const check = (value: unknown, probe: boolean) => referenceValues(value).filter(({ pointer, invalid }) => {
      if (invalid) return true;
      const [root, id] = pointer.slice(1).split("/").map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~"));
      return root === "steps" ? !id || !upstream.get(step.id)?.has(id) : root === "probe" && !probe;
    });
    const pointers = step.kind === "action" ? check(step.action.arguments, false)
      : step.kind === "condition" ? check(step.predicate, false)
      : step.kind === "poll" ? [...check(step.probe.arguments, false), ...check(step.predicate, true)]
      : step.kind === "waitUntil" ? [...check(step.probe.arguments, false), ...check(step.successWhen, true), ...check(step.failureWhen, true)] : [];
    return pointers.filter((ref, index) => pointers.findIndex((other) => other.pointer === ref.pointer && other.invalid === ref.invalid) === index).map(({ pointer, invalid }) => ({ stepId: step.id, name: step.name, pointer, key: invalid ? "flow.invalidReference" : "flow.unsafeReference" }));
  });
}

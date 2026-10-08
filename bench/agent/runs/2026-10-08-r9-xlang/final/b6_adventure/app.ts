export interface Room {
  readonly name: string;
  readonly exits: readonly (readonly [string, string])[];
  readonly items: readonly string[];
  readonly locked: readonly string[];
}

export interface Game {
  readonly room: string;
  readonly rooms: readonly Room[];
  readonly inventory: readonly string[];
  readonly moves: number;
}

export function room(name: string, exits: readonly (readonly [string, string])[], items: readonly string[], locked: readonly string[]): Room {
  return { name, exits, items, locked };
}

export function world(): Room[] {
  return [
    room("hall", [["north", "vault"], ["east", "kitchen"]], [], ["north"]),
    room("kitchen", [["west", "hall"]], ["key", "bread"], []),
    room("vault", [["south", "hall"]], ["gold"], []),
  ];
}

export function newGame(): Game {
  return { room: "hall", rooms: world(), inventory: [], moves: 0 };
}

export function current(g: Game): Room {
  const r = g.rooms.find((r) => r.name === g.room);
  if (r === undefined) throw new Error(`no room ${g.room}`);
  return r;
}

function sorted(xs: readonly string[]): string[] {
  return [...xs].sort();
}

export function look(g: Game): string {
  const r = current(g);
  let s = `You are in the ${r.name}. Exits: ` + sorted(r.exits.map(([d]) => d)).join(", ");
  if (r.items.length > 0) s += ". Items: " + sorted(r.items).join(", ");
  return s + ".";
}

export function go(g: Game, dir: string): [Game, string] {
  const r = current(g);
  for (const [d, target] of r.exits) {
    if (d === dir) {
      if (r.locked.includes(d) && !g.inventory.includes("key")) return [g, "The door is locked."];
      const g2 = { ...g, room: target, moves: g.moves + 1 };
      return [g2, look(g2)];
    }
  }
  return [g, "You can't go that way."];
}

function withItems(g: Game, roomItems: readonly string[], inventory: readonly string[]): Game {
  return {
    ...g,
    inventory,
    rooms: g.rooms.map((r) => (r.name === g.room ? { ...r, items: roomItems } : r)),
  };
}

export function take(g: Game, item: string): [Game, string] {
  const r = current(g);
  if (!r.items.includes(item)) return [g, `There is no ${item} here.`];
  return [withItems(g, r.items.filter((i) => i !== item), [...g.inventory, item]), "Taken."];
}

export function drop(g: Game, item: string): [Game, string] {
  if (!g.inventory.includes(item)) return [g, `You don't have ${item}.`];
  return [withItems(g, [...current(g).items, item], g.inventory.filter((i) => i !== item)), "Dropped."];
}

export function score(g: Game): number {
  return 10 * g.inventory.length + (g.room === "vault" ? 25 : 0);
}

export function step(g: Game, cmd: string): [Game, string] {
  const w = cmd.trim().toLowerCase().split(/\s+/).filter((x) => x !== "");
  if (w.length === 1 && w[0] === "look") return [g, look(g)];
  if (w.length === 1 && w[0] === "inventory") {
    return [g, g.inventory.length === 0 ? "You carry nothing." : `You carry: ${sorted(g.inventory).join(", ")}.`];
  }
  if (w.length === 2) {
    if (w[0] === "go") return go(g, w[1]);
    if (w[0] === "take") return take(g, w[1]);
    if (w[0] === "drop") return drop(g, w[1]);
  }
  return [g, "I don't understand."];
}

export function run(cmds: readonly string[]): [Game, string[]] {
  let g = newGame();
  const out: string[] = [];
  for (const c of cmds) {
    const [g2, msg] = step(g, c);
    g = g2;
    out.push(msg);
  }
  return [g, out];
}

export function play(cmds: readonly string[]): string[] {
  return run(cmds)[1];
}

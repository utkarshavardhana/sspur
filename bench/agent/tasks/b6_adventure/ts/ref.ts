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

export function room(
  name: string,
  exits: readonly (readonly [string, string])[],
  items: readonly string[],
  locked: readonly string[],
): Room {
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

export function look(g: Game): string {
  const r = current(g);
  const items = r.items.length > 0 ? ` Items: ${[...r.items].sort().join(", ")}.` : "";
  return `You are in the ${r.name}. Exits: ` + r.exits.map(([d]) => d).sort().join(", ") + "." + items;
}

export function go(g: Game, dir: string): [Game, string] {
  for (const [d, target] of current(g).exits) {
    if (d === dir) {
      if (current(g).locked.includes(dir) && !g.inventory.includes("key")) return [g, "The door is locked."];
      const g2 = { ...g, room: target, moves: g.moves + 1 };
      return [g2, look(g2)];
    }
  }
  return [g, "You can't go that way."];
}

function setItems(g: Game, items: readonly string[]): Game {
  return { ...g, rooms: g.rooms.map((r) => (r.name === g.room ? { ...r, items } : r)) };
}

export function take(g: Game, item: string): [Game, string] {
  if (current(g).items.includes(item)) {
    const g2 = setItems(g, current(g).items.filter((i) => i !== item));
    return [{ ...g2, inventory: [...g.inventory, item] }, "Taken."];
  }
  return [g, `There is no ${item} here.`];
}

export function drop(g: Game, item: string): [Game, string] {
  if (g.inventory.includes(item)) {
    const g2 = setItems(g, [...current(g).items, item]);
    return [{ ...g2, inventory: g.inventory.filter((i) => i !== item) }, "Dropped."];
  }
  return [g, `You don't have ${item}.`];
}

export function step(g: Game, cmd: string): [Game, string] {
  const w = cmd.toLowerCase().split(/\s+/).filter((s) => s !== "");
  if (w.length === 1 && w[0] === "look") return [g, look(g)];
  if (w.length === 1 && w[0] === "inventory") {
    return [g, g.inventory.length > 0 ? "You carry: " + [...g.inventory].sort().join(", ") + "." : "You carry nothing."];
  }
  if (w.length === 2 && w[0] === "go") return go(g, w[1]);
  if (w.length === 2 && w[0] === "take") return take(g, w[1]);
  if (w.length === 2 && w[0] === "drop") return drop(g, w[1]);
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

export function score(g: Game): number {
  return 10 * g.inventory.length + (g.room === "vault" ? 25 : 0);
}

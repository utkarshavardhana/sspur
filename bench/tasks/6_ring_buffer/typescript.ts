export class RingBuffer<T> {
  private buf: (T | undefined)[];
  private head = 0;
  private size = 0;

  constructor(private readonly capacity: number) {
    this.buf = new Array(capacity);
  }

  get length(): number {
    return this.size;
  }

  push(x: T): boolean {
    if (this.size === this.capacity) return false;
    this.buf[(this.head + this.size) % this.capacity] = x;
    this.size++;
    return true;
  }

  pop(): T | undefined {
    if (this.size === 0) return undefined;
    const x = this.buf[this.head];
    this.buf[this.head] = undefined;
    this.head = (this.head + 1) % this.capacity;
    this.size--;
    return x;
  }
}

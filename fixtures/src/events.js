// fixtures/src/events.js — JavaScript fixture (T037's first-class twelve).

export class Watchable {
    #listeners = new Set();

    watch(fn) {
        this.#listeners.add(fn);
        return () => this.#listeners.delete(fn);
    }

    push(value) {
        for (const fn of this.#listeners) {
            fn(value);
        }
    }
}

export function debounce(fn, wait) {
    let timer = null;
    return (...args) => {
        clearTimeout(timer);
        timer = setTimeout(() => fn(...args), wait);
    };
}

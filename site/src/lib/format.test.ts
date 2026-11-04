import { describe, expect, it } from 'vitest';
import { pct, record, shortDate, signed, spread } from './format';

describe('format', () => {
	it('signs ratings with a true minus and no negative zero', () => {
		expect(signed(12.34)).toBe('+12.3');
		expect(signed(-4)).toBe('−4.0');
		expect(signed(-0.04)).toBe('0.0');
	});
	it('clamps extreme probabilities', () => {
		expect(pct(0.001)).toBe('<1%');
		expect(pct(0.999)).toBe('>99%');
		expect(pct(0.5)).toBe('50%');
		expect(pct(1)).toBe('100%');
	});
	it('names the favorite in a spread', () => {
		expect(spread(5.5, 'LSU', 'Alabama')).toBe('LSU −5.5');
		expect(spread(-3, 'LSU', 'Alabama')).toBe('Alabama −3.0');
		expect(spread(0, 'A', 'B')).toBe('Pick');
	});
	it('formats records and dates', () => {
		expect(record(9, 2)).toBe('9–2');
		expect(shortDate('2026-11-07')).toBe('Nov 7');
	});
});

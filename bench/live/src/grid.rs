//! 範囲で選ぶための索引。段ごとにセルの大きさが 2 倍になる格子で、要素は、囲む箱より大きいセルを持つ
//! いちばん細かい段の、箱の中心があるセルに入る。セルの中の要素は両方向のリストでつなぐので、
//! 足す・除く・動かすのに確保が要らず、手間は要素の数によらない。

/// つながりの終わり
const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Level {
    /// セルの辺の長さ
    size: f64,
    /// 横のセルの数
    columns: u32,
    /// 縦のセルの数
    rows: u32,
    /// `head` の中の、この段の最初のセル
    offset: u32,
}

/// 格子の索引
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grid {
    origin: [f64; 2],
    levels: Vec<Level>,
    /// セルごとの最初の要素
    head: Vec<u32>,
    /// 要素ごとの、同じセルの次の要素
    next: Vec<u32>,
    /// 要素ごとの、同じセルの前の要素
    prev: Vec<u32>,
    /// 要素が入っているセル（`head` の番号）
    cell: Vec<u32>,
}

/// 箱が空（どの点も含まない）か。数でない値を持つ箱も空とする
fn empty([x0, y0, x1, y1]: [f64; 4]) -> bool {
    !(x0 <= x1 && y0 <= y1)
}

impl Grid {
    /// 要素の箱（最小の x・y、最大の x・y）から作る
    pub fn build(boxes: &[[f64; 4]]) -> Grid {
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for item in boxes.iter().filter(|item| !empty(**item)) {
            bounds = [
                bounds[0].min(item[0]),
                bounds[1].min(item[1]),
                bounds[2].max(item[2]),
                bounds[3].max(item[3]),
            ];
        }
        if empty(bounds) {
            bounds = [0.0, 0.0, 1.0, 1.0];
        }
        let longer = (bounds[2] - bounds[0]).max(bounds[3] - bounds[1]);
        let longer = if longer > 0.0 && longer.is_finite() {
            longer
        } else {
            1.0
        };
        // 細長い範囲でも、セルの数が要素の数ほどになるようにする
        let width = (bounds[2] - bounds[0]).max(longer * 1e-6);
        let height = (bounds[3] - bounds[1]).max(longer * 1e-6);
        let count = boxes.len().max(1) as f64;
        let finest = (width * height / count).sqrt().max(longer / 32768.0);
        let mut levels = Vec::new();
        let mut size = finest;
        let mut offset = 0u32;
        loop {
            let columns = ((width / size).ceil() as u32).max(1);
            let rows = ((height / size).ceil() as u32).max(1);
            levels.push(Level {
                size,
                columns,
                rows,
                offset,
            });
            offset += columns * rows;
            if columns == 1 && rows == 1 {
                break;
            }
            size *= 2.0;
        }
        let mut grid = Grid {
            origin: [bounds[0], bounds[1]],
            levels,
            head: vec![NONE; offset as usize],
            next: vec![NONE; boxes.len()],
            prev: vec![NONE; boxes.len()],
            cell: vec![NONE; boxes.len()],
        };
        for (row, item) in boxes.iter().enumerate() {
            grid.insert(row as u32, *item);
        }
        grid
    }

    /// 段 `level` の、座標 `value` が入る列か行。範囲の外は、端のセルに寄せる
    fn index(&self, level: &Level, axis: usize, value: f64) -> u32 {
        let count = if axis == 0 { level.columns } else { level.rows };
        let at = ((value - self.origin[axis]) / level.size).floor();
        // 数でない値は 0 になる
        (at.max(0.0) as u32).min(count - 1)
    }

    /// 箱が入るセル
    fn place(&self, item: [f64; 4]) -> Option<u32> {
        if empty(item) {
            return None;
        }
        let extent = (item[2] - item[0]).max(item[3] - item[1]);
        let last = self.levels.len() - 1;
        let at = self
            .levels
            .iter()
            .position(|level| level.size >= extent)
            .unwrap_or(last);
        let level = self.levels.get(at)?;
        let column = self.index(level, 0, (item[0] + item[2]) / 2.0);
        let row = self.index(level, 1, (item[1] + item[3]) / 2.0);
        Some(level.offset + row * level.columns + column)
    }

    fn insert(&mut self, row: u32, item: [f64; 4]) {
        let Some(cell) = self.place(item) else {
            return;
        };
        let first = self.head[cell as usize];
        self.next[row as usize] = first;
        self.prev[row as usize] = NONE;
        if first != NONE {
            self.prev[first as usize] = row;
        }
        self.head[cell as usize] = row;
        self.cell[row as usize] = cell;
    }

    fn remove(&mut self, row: u32) {
        let cell = self.cell[row as usize];
        if cell == NONE {
            return;
        }
        let (before, after) = (self.prev[row as usize], self.next[row as usize]);
        if before == NONE {
            self.head[cell as usize] = after;
        } else {
            self.next[before as usize] = after;
        }
        if after != NONE {
            self.prev[after as usize] = before;
        }
        self.cell[row as usize] = NONE;
    }

    /// 終わりに要素を足す。段の作りは、開いたときのまま
    pub fn push(&mut self, item: [f64; 4]) {
        let row = self.cell.len() as u32;
        self.next.push(NONE);
        self.prev.push(NONE);
        self.cell.push(NONE);
        self.insert(row, item);
    }

    /// 終わりの要素を除く
    pub fn pop(&mut self) {
        let Some(row) = self.cell.len().checked_sub(1) else {
            return;
        };
        self.remove(row as u32);
        self.next.pop();
        self.prev.pop();
        self.cell.pop();
    }

    /// 要素の箱が変わった。空の箱（除いた要素）は、索引から外す
    pub fn update(&mut self, row: u32, item: [f64; 4]) {
        if row as usize >= self.cell.len() {
            return;
        }
        if self.place(item) == Some(self.cell[row as usize]) {
            return;
        }
        self.remove(row);
        self.insert(row, item);
    }

    /// 箱が `rect` と重なる要素を、行の順に `out` へ足す。`boxes` は、今の箱
    pub fn query(&self, boxes: &[[f64; 4]], rect: [f64; 4], out: &mut Vec<u32>) {
        let start = out.len();
        if empty(rect) {
            return;
        }
        for level in &self.levels {
            // この段の要素は、中心からセルの半分までしか広がらない。丸めの分だけ余分に見る
            let reach = level.size * 0.5625;
            let columns =
                self.index(level, 0, rect[0] - reach)..=self.index(level, 0, rect[2] + reach);
            let rows =
                self.index(level, 1, rect[1] - reach)..=self.index(level, 1, rect[3] + reach);
            for row in rows {
                for column in columns.clone() {
                    let mut at = self.head[(level.offset + row * level.columns + column) as usize];
                    while at != NONE {
                        if let Some(item) = boxes.get(at as usize)
                            && item[0] <= rect[2]
                            && item[2] >= rect[0]
                            && item[1] <= rect[3]
                            && item[3] >= rect[1]
                        {
                            out.push(at);
                        }
                        at = self.next[at as usize];
                    }
                }
            }
        }
        out.get_mut(start..).unwrap_or_default().sort_unstable();
    }

    /// 索引のバイト数
    pub fn bytes(&self) -> usize {
        (self.head.len() + self.next.len() * 3) * 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 決まった列を返す乱数
    struct Random(u64);

    impl Random {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    fn brute(boxes: &[[f64; 4]], rect: [f64; 4]) -> Vec<u32> {
        (0..boxes.len() as u32)
            .filter(|&row| {
                let item = boxes[row as usize];
                !empty(item)
                    && item[0] <= rect[2]
                    && item[2] >= rect[0]
                    && item[1] <= rect[3]
                    && item[3] >= rect[1]
            })
            .collect()
    }

    #[test]
    fn queries_match_a_full_scan_before_and_after_moves() {
        let mut random = Random(7);
        let mut boxes: Vec<[f64; 4]> = (0..3000)
            .map(|index| {
                let (x, y) = (random.next() * 200.0, random.next() * 100.0);
                // 小さいもの・大きいもの・点を混ぜる
                let size = match index % 7 {
                    0 => 0.0,
                    1 => random.next() * 80.0,
                    _ => random.next() * 0.5,
                };
                [x, y, x + size, y + size * random.next()]
            })
            .collect();
        let mut grid = Grid::build(&boxes);
        let check = |grid: &Grid, boxes: &[[f64; 4]], random: &mut Random| {
            for _ in 0..200 {
                let (x, y) = (random.next() * 260.0 - 30.0, random.next() * 160.0 - 30.0);
                let rect = [x, y, x + random.next() * 20.0, y + random.next() * 20.0];
                let mut found = Vec::new();
                grid.query(boxes, rect, &mut found);
                assert_eq!(found, brute(boxes, rect));
            }
        };
        check(&grid, &boxes, &mut random);
        // 動かす。範囲の外へ出るものもある
        for _ in 0..2000 {
            let row = (random.next() * boxes.len() as f64) as usize;
            let (dx, dy) = (random.next() * 400.0 - 200.0, random.next() * 400.0 - 200.0);
            let item = boxes[row];
            boxes[row] = [item[0] + dx, item[1] + dy, item[2] + dx, item[3] + dy];
            grid.update(row as u32, boxes[row]);
        }
        check(&grid, &boxes, &mut random);
        let mut all = Vec::new();
        grid.query(&boxes, [-1e9, -1e9, 1e9, 1e9], &mut all);
        assert_eq!(all.len(), boxes.len());
    }

    #[test]
    fn a_single_point_and_an_empty_set_work() {
        let grid = Grid::build(&[]);
        let mut found = Vec::new();
        grid.query(&[], [0.0, 0.0, 1.0, 1.0], &mut found);
        assert!(found.is_empty());
        let boxes = [[5.0, 5.0, 5.0, 5.0]];
        let grid = Grid::build(&boxes);
        grid.query(&boxes, [4.0, 4.0, 6.0, 6.0], &mut found);
        assert_eq!(found, [0]);
    }
}

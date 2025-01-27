use delaunator::{next_halfedge, Point, Triangulation};

use crate::{ConvexBoundary, Voronoi};

pub(crate) const EQ_EPSILON: f64 = 4. * std::f64::EPSILON;

/// Gets the index of the triangle (starting half-edge) this half-edge belongs to.
#[inline]
pub fn triangle_of_edge(edge: usize) -> usize {
    edge / 3
}

/// Returns the index to the site that half-edge `e` points to.
/// This is similar to `triangles`. Given an half-edge `e`, `triangles` returns the index of the site the half-edge start off. `site_of_incoming` returns the index of the site the half-edge points to.
#[inline]
pub fn site_of_incoming(triangulation: &Triangulation, e: usize) -> usize {
    triangulation.triangles[next_halfedge(e)]
}

/// Gets the delaunay edge associated with a voronoi edge where ```a``` and ```b``` are the index of the triangle whose circumcenter representes the vertices of the voronoi edge.
///
/// The returned value is a delaunay edge or EMPTY if the voronoi edge does not exist (or was clipped and vertices do not represent circumcenters).
#[allow(dead_code)]
pub fn delaunay_edge_from_voronoi_edge(triangulation: &Triangulation, a: usize, b: usize) -> usize {
    // get delaunay edges of triangles associated with circumcenters a and b
    let mut ta = a * 3;
    let mut tb = b * 3;

    if ta < triangulation.triangles.len() && tb < triangulation.triangles.len() {
        // find the common delaunay edge
        for _ in 0..3 {
            for _ in 0..3 {
                if ta == triangulation.halfedges[tb] {
                    return ta;
                } else {
                    tb = delaunator::next_halfedge(tb);
                }
            }
            ta = delaunator::next_halfedge(ta);
        }
    }

    delaunator::EMPTY
}

pub fn calculate_approximated_cetroid<'a>(points: impl Iterator<Item = &'a Point>) -> Point {
    let mut r = Point { x: 0.0, y: 0.0 };
    let mut n = 0;
    for p in points {
        r.x += p.x;
        r.y += p.y;
        n += 1;
    }

    let n = n as f64;
    r.x /= n;
    r.y /= n;

    r
}

pub fn cicumcenter(a: &Point, b: &Point, c: &Point) -> Point {
    // move origin to a
    let b_x = b.x - a.x;
    let b_y = b.y - a.y;
    let c_x = c.x - a.x;
    let c_y = c.y - a.y;

    let bb = b_x * b_x + b_y * b_y;
    let cc = c_x * c_x + c_y * c_y;
    let d = 1.0 / (2.0 * (b_x * c_y - b_y * c_x));

    Point {
        x: a.x + d * (c_y * bb - b_y * cc),
        y: a.y + d * (b_x * cc - c_x * bb),
    }
}

/// Calculates the squared distance between a and b
pub fn dist2(a: &Point, b: &Point) -> f64 {
    let x = a.x - b.x;
    let y = a.y - b.y;
    (x * x) + (y * y)
}

#[inline]
pub fn abs_diff_eq(a: f64, b: f64, epsilon: f64) -> bool {
    (if a > b { a - b } else { b - a }) <= epsilon
}

/// Given a voronoi and two sites, returns whether they share a common voronoi edge.
pub fn has_common_voronoi_edge<T: ConvexBoundary>(
    voronoi: &Voronoi<T>,
    a: usize,
    b: usize,
) -> bool {
    let mut common = 0;
    for &ta in voronoi.cell(a).triangles() {
        for &tb in voronoi.cell(b).triangles() {
            if ta == tb {
                common += 1;
                break;
            }
        }
    }

    common >= 2
}

#[cfg(test)]
pub(crate) mod test {
    use delaunator::Point;

    use crate::{BoundingBox, ConvexBoundary, Voronoi, VoronoiBuilder};

    pub fn validate_voronoi<T: ConvexBoundary>(voronoi: &Voronoi<T>) {
        for cell in voronoi.iter_cells() {
            let vertices: Vec<Point> = cell.iter_vertices().cloned().collect();

            let area = calculate_area(&vertices);
            if area <= 0. {
                fail(
                    &voronoi,
                    format!(
                        "Cell {}: not counter-clockwise. Area is {area}. {:?}",
                        cell.site(),
                        cell.triangles().iter().copied().collect::<Vec<usize>>()
                    ),
                );
            }

            vertices
                .iter()
                .enumerate()
                .filter(|(_, p)| !voronoi.boundary().is_inside(p))
                .for_each(|(v, p)| {
                    fail(
                        &voronoi,
                        format!(
                            "Cell {}: vertex {v} {:?} is outside diagram boundary.",
                            cell.site(),
                            p
                        ),
                    );
                });

            if !is_convex(&vertices) {
                fail(
                    &voronoi,
                    format!(
                        "Cell {} is not convex. {:?}",
                        cell.site(),
                        cell.triangles().iter().copied().collect::<Vec<usize>>()
                    ),
                );
            }

            if !is_point_inside(&vertices, cell.site_position()) {
                fail(
                    &voronoi,
                    format!(
                        "Cell {} site is outside the voronoi cell. {:?}",
                        cell.site(),
                        cell.triangles().iter().copied().collect::<Vec<usize>>()
                    ),
                );
            }
        }

        for corner in voronoi.boundary().vertices() {
            let mut inside = false;
            for cell in voronoi.iter_cells() {
                let cell_vertices = cell.iter_vertices().cloned().collect();
                if is_point_inside(&cell_vertices, &corner) {
                    inside = true;
                    break;
                }
            }

            if !inside {
                fail(
                    &voronoi,
                    format!("Corner {:?} is not inside any hull cell.", &corner),
                );
            }
        }
    }

    pub fn new_voronoi_builder_from_asset(
        asset: &str,
    ) -> std::io::Result<VoronoiBuilder<BoundingBox>> {
        let basepath = "examples/assets/";

        let file = std::fs::File::open(basepath.to_string() + asset)?;
        let sites: Vec<[f64; 2]> = serde_json::from_reader(file)?;
        let sites: Vec<Point> = sites.iter().map(|&[x, y]| Point { x, y }).collect();

        let mut center = sites.iter().fold(Point { x: 0., y: 0. }, |acc, p| Point {
            x: acc.x + p.x,
            y: acc.y + p.y,
        });
        center.x /= sites.len() as f64;
        center.y /= sites.len() as f64;

        let farthest_distance = sites
            .iter()
            .map(|p| {
                let (x, y) = (center.x - p.x, center.y - p.y);
                x * x + y * y
            })
            .reduce(f64::max)
            .unwrap()
            .sqrt();

        Ok(VoronoiBuilder::default()
            .set_sites(sites)
            .set_boundary(BoundingBox::new(
                center,
                farthest_distance * 2.0,
                farthest_distance * 2.0,
            )))
    }

    pub fn assert_list_eq<T>(expected: &[T], actual: &[T], message: &str)
    where
        T: std::fmt::Debug + Eq,
    {
        assert_eq!(
            expected.len(),
            actual.len(),
            "Lists do not have same length. {} Expected: {:?}, Actual: {:?}",
            message,
            expected,
            actual
        );
        for i in 0..expected.len() {
            assert_eq!(
                expected[i], actual[i],
                "Elements differ at index {i}. {} Expected: {:?}, Actual: {:?}",
                message, expected, actual
            );
        }
    }

    fn fail<T: ConvexBoundary>(voronoi: &Voronoi<T>, message: String) {
        let path = "test_sites.json";
        let s = format!("{:?}", voronoi.sites());
        std::io::Write::write_all(&mut std::fs::File::create(path).unwrap(), s.as_bytes()).unwrap();
        panic!(
            "Voronoi validation failed. Wrote sites to file '{}'. {}",
            path, message
        );
    }

    fn is_convex(vertices: &Vec<Point>) -> bool {
        let triangulation = delaunator::triangulate(vertices);
        triangulation.hull.len() == vertices.len()
    }

    /// Checks whether ```inside``` is inside convex polygon ```vertices``` ordered counter-clockwise
    fn is_point_inside(vertices: &Vec<Point>, inside: &Point) -> bool {
        for (a, b) in vertices.iter().zip(vertices.iter().cycle().skip(1)) {
            if robust::orient2d(a.into(), b.into(), inside.into()) > 0. {
                return false;
            }
        }

        true
    }

    /// Check that the cell is ordered counter-clockwise and inside the bounding geometry.
    fn calculate_area(vertices: &Vec<Point>) -> f64 {
        vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .fold(0.0, |acc, (a, b)| acc + ((b.x - a.x) * (b.y + a.y)))
    }
}

use itertools::Itertools;
use nalgebra::DVector;
use std::time::Instant;

fn pairwise_sumation(input: &[f64]) -> f64 {
    let sub_array_size = 32;
    let vec_len = input.len();
    if vec_len < sub_array_size {
        let mut sum = 0.;
        for val in input {
            sum += *val;
        }
        sum
    } else {
        let new_size = vec_len / 2;
        pairwise_sumation(&input[..new_size]) + pairwise_sumation(&input[new_size..vec_len])
    }
}

// fn pairwise_sum_matrix(input: &DVectorView<f64>) -> f64 {
//     let vec_len = input.len();
//     if vec_len < 64 {
//         input.sum()
//     } else {
//         let new_size = vec_len / 2;
//         pairwise_sum_matrix(&input.rows(0, new_size))
//             + pairwise_sum_matrix(&input.rows(new_size, vec_len - new_size))
//     }
// }

fn kahansum2(input: Vec<f64>) -> f64 {
    // Prepare the accumulator.
    // Prepare the accumulator.
    let mut sum = 0.0;
    // A running compensation for lost low-order bits.
    let mut c = 0.0;
    // The array input has elements indexed input[1] to input[input.length].
    for i in input.iter() {
        // c is zero the first time around.
        let y = i - c;
        // Alas, sum is big, y small, so low-order digits of y are lost.
        let t = sum + y;
        // (t - sum) cancels the high-order part of y;
        // subtracting y recovers negative (low part of y)
        c = (t - sum) - y;
        // Algebraically, c should always be zero. Beware
        // overly-aggressive optimizing compilers!
        sum = t;
        // Next time around, the lost low part will be added to y in a fresh attempt.
    }
    return sum;
}

fn kahansum_vector(input: &DVector<f64>) -> f64 {
    // Prepare the accumulator.
    // Prepare the accumulator.
    let mut sum = 0.0;
    // A running compensation for lost low-order bits.
    let mut c = 0.0;
    // The array input has elements indexed input[1] to input[input.length].
    for i in input {
        // c is zero the first time around.
        let y = i - &c;
        // Alas, sum is big, y small, so low-order digits of y are lost.
        let t = &sum + &y;
        // (t - sum) cancels the high-order part of y;
        // subtracting y recovers negative (low part of y)
        c = (&t - &sum) - &y;
        // Algebraically, c should always be zero. Beware
        // overly-aggressive optimizing compilers!
        sum = t;
        // Next time around, the lost low part will be added to y in a fresh attempt.
    }
    return sum;
}

fn main() {
    // let mat = DVector::from(vec![-3., -2., -1., 0., 1., 2., 3.]);
    // let mat2 = MatrixXx1::from(vec![-3., f64::NAN, -2., -1., 1., f64::INFINITY, 3.]);
    // let mat22 = MatrixXx1::from(vec![f64::NAN, f64::INFINITY]);
    // let mat3 = MatrixXx1::from(vec![f64::INFINITY, -2., -1., 0., 1., 2., 3.]);
    // let mat4 = MatrixXx1::from(vec![f64::NAN, f64::NAN]);
    // let infinite_plus = f64::INFINITY;
    // let infinite_minus = -f64::INFINITY;
    // let mut vec = Vec::<f64>::new();

    // let test = mat.rows(0, 5);
    // let test = mat.fixed_columns

    // let mat_wo_inf =  mat2.iter().filter_map(|x| {
    //     if !x.is_nan() & x.is_finite(){
    //         Some(x.clone())
    //     }
    //     else{
    //         None
    //     }
    // }).collect::<Vec<f64>>();

    // let mat_wo_inf = MatrixXx1::from(
    //     mat22
    //         .iter()
    //         .cloned()
    //         .filter(|x| !x.is_nan() & x.is_finite())
    //         .collect::<Vec<f64>>(),
    // );
    // let bla: Vec<&f64> = mat_wo_inf.to_vec().clone();
    // let test = MatrixXx1::from_vec(bla);
    // let mat_new = MatrixXx1::from_vec(mat_wo_inf.clone());

    // println!("{mat2}");
    // println!("{mat_wo_inf}");
    // println!("{}", mat_wo_inf.len());

    // // println!("min:\t{}\nmax:\t{}\n", mat.min(), mat.max());
    // println!("min:\t{}\nmax:\t{}\n", mat2.min(), mat2.max());
    // println!("min:\t{}\nmax:\t{}\n", mat3.min(), mat3.max());
    // println!("min:\t{}\nmax:\t{}\n", mat4.min(), mat4.max());

    // println!("min:\t{}\nmax:\t{}\n", mat_wo_inf.min(), mat_wo_inf.max());
    let s1 = 1000.100;
    let s2 = 999.;
    let test = (s1 - s2 - 1.1) / s1;
    let test = format!("{:+e}", test);
    println!("{test}");

    let energy = 1. + f64::EPSILON;
    let num_rays = 1000001;

    let energy_per_ray = energy * 1. / num_rays as f64;

    let mut rays_vec2 = Vec::<f64>::with_capacity(num_rays);
    for _ in 0..(num_rays as usize) {
        rays_vec2.push(energy_per_ray)
    }

    let mat = DVector::from_vec(rays_vec2.clone());
    let rays_vec1 = rays_vec2.clone();
    let rays_vec3 = rays_vec2.clone();
    let energy_vec = rays_vec2.iter().fold(0., |a, b| a + b);

    let start = Instant::now();
    let energy_vec0 = kahansum_vector(&mat); //sum_with_accumulator::<NaiveSum<f64>>();
    let duration = start.elapsed();
    println!(
        "Time elapsed in kahan matrix summation() is: {:?}",
        duration
    );

    let start = Instant::now();
    let energy_vec1 = rays_vec2.iter().cloned().tree_reduce(|a, b| a + b); //sum_with_accumulator::<NaiveSum<f64>>();
    let duration = start.elapsed();
    println!(
        "Time elapsed in pariwise itertools summation() is: {:?}",
        duration
    );

    let start = Instant::now();
    let energy_vec2 = kahansum2(rays_vec1);
    let duration = start.elapsed();
    println!("Time elapsed in kahan summation() is: {:?}", duration);

    let start = Instant::now();
    let energy_vec3 = pairwise_sumation(&rays_vec3[..]);
    let duration = start.elapsed();
    println!(
        "Time elapsed in pairwise self written summation() is: {:?}",
        duration
    );

    println!("energy:{}", energy_vec0);
    println!("energy:{}", energy_vec);
    println!("energy:{}", energy_vec1.unwrap());
    println!("energy:{}", energy_vec2);
    println!("energy:{}", energy_vec3);

    // println!("energy:{}", energy_vec);
    // println!("energy:{}", energy_vec2);
    // println!("energy:{}", energy_vec3);
    // let mut summed_time = Duration::new(0, 0);
    // for i in 1..1000{
    //     let energy = (i as f64)+f64::EPSILON;
    //     let num_rays = 1000 * i;
    //     let energy_per_ray = energy/num_rays as f64;
    //     let mut rays_vec = Vec::<f64>::with_capacity(num_rays);
    //     for i in 0..(num_rays as usize){
    //         rays_vec.push(energy_per_ray)
    //     }
    //     let start = Instant::now();
    //     let _ = kahansum2(rays_vec);
    //     let duration = start.elapsed();
    //     summed_time += duration;

    // }
    // println!("Time elapsed in kahan summation() is: {:?}", summed_time);

    // let mut summed_time = Duration::new(0, 0);
    // for i in 1..1000{
    //     let energy = (i as f64)+f64::EPSILON;
    //     let num_rays = 1000 * i;
    //     let energy_per_ray = energy/num_rays as f64;
    //     let mut rays_vec = Vec::<f64>::with_capacity(num_rays);
    //     for i in 0..(num_rays as usize){
    //         rays_vec.push(energy_per_ray)
    //     }
    //     let start = Instant::now();
    //     let _ = pairwise_sumation(&rays_vec);
    //     let duration = start.elapsed();
    //     summed_time += duration;

    // }
    // println!("Time elapsed in pairwise summation() is: {:?}", summed_time);
}

mod tests {
    use crate::logic::tests::vm_logic_builder::{TestVMLogic, VMLogicBuilder};
    use crate::logic::MemSlice;
    use amcl::bls381::bls381::utils::{
        serialize_g1, serialize_g2, serialize_uncompressed_g1, serialize_uncompressed_g2,
        subgroup_check_g1, subgroup_check_g2,
    };
    use amcl::bls381::{big::Big, ecp::ECP, ecp2::ECP2, fp2::FP2, pair};
    use amcl::rand::RAND;
    use ark_bls12_381::{Fr, Fq, Fq2, G1Affine, G2Affine};
    use ark_ec::bls12::Bls12Config;
    use ark_ec::hashing::curve_maps::wb::WBMap;
    use ark_ec::hashing::map_to_curve_hasher::MapToCurve;
    use ark_ec::AffineRepr;
    use ark_ec::CurveGroup;
    use ark_ff::Field;
    use ark_ff::PrimeField;
    use ark_serialize::CanonicalSerialize;
    use ark_serialize::CanonicalDeserialize;
    use ark_serialize::CanonicalSerializeWithFlags;
    use ark_serialize::EmptyFlags;
    use ark_std::{test_rng, UniformRand, One, Zero};
    use rand::{seq::SliceRandom, thread_rng, Rng, RngCore};
    use rand::distributions::Distribution;
    use std::fs;
    use std::str::FromStr;
    use std::ops::{Mul, Neg, Add};

    const P: &str = "1a0111ea397fe69a4b1ba7b6434bacd764774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaab";
    const P_MINUS_1: &str = "4002409555221667393417789825735904156556882819939007885332058136124031650490837864442687629129015664037894272559786";
    const R: &str = "52435875175126190479447740508185965837690552500527637822603658699938581184513";
    const R_MINUS_1: &str = "73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000";

    const TESTS_ITERATIONS: usize = 100;
    const MAX_N_PAIRING: usize = 105;

    macro_rules! run_bls12381_fn {
        ($fn_name:ident, $buffer:expr, $expected_res:expr) => {{
            let mut logic_builder = VMLogicBuilder::default();
            let mut logic = logic_builder.build();
            let input = logic.internal_mem_write($buffer.concat().as_slice());
            let res = logic.$fn_name(input.len, input.ptr, 0).unwrap();
            assert_eq!(res, $expected_res);
        }};
        ($fn_name:ident, $buffer:expr) => {{
            let mut logic_builder = VMLogicBuilder::default();
            let mut logic = logic_builder.build();
            let input = logic.internal_mem_write($buffer.concat().as_slice());
            let res = logic.$fn_name(input.len, input.ptr, 0).unwrap();
            assert_eq!(res, 0);
            logic.registers().get_for_free(0).unwrap().to_vec()
        }};
    }

    struct G1Operations;
    struct G2Operations;

    fn get_381bit_big(rnd: &mut RAND) -> Big {
        let mut r: Big = Big::random(rnd);
        r.mod2m(381);
        r
    }

    impl G1Operations {
        const POINT_LEN: usize = 96;
        const MAX_N_SUM: usize = 675;
        const MAX_N_MULTIEXP: usize = 500;
        const MAX_N_MAP: usize = 500;
        const MAX_N_DECOMPRESS: usize = 500;

        fn get_random_curve_point(rnd: &mut RAND) -> ECP {
            loop {
                let p: ECP = ECP::new_big(&get_381bit_big(rnd));

                if !p.is_infinity() {
                    return p;
                }
            }
        }

        fn _get_random_fp<R: Rng + ?Sized>(rng: &mut R) -> Fq {
            Fq::rand(rng)
        }

        fn serialize_fp(fq: &Fq) -> Vec<u8> {
            let mut result = [0u8; 48];
            let rep = fq.into_bigint();
            for i in 0..6 {
                result[i * 8..(i + 1) * 8].copy_from_slice(&rep.0[5 - i].to_be_bytes());
            }
            result.to_vec()
        }
    }

    impl G2Operations {
        const POINT_LEN: usize = 192;
        const MAX_N_SUM: usize = 338;
        const MAX_N_MULTIEXP: usize = 250;
        const MAX_N_MAP: usize = 250;
        const MAX_N_DECOMPRESS: usize = 250;

        fn get_random_curve_point(rnd: &mut RAND) -> ECP2 {
            loop {
                let p: ECP2 = ECP2::new_fp2(&Self::get_random_fp(rnd));
                if !p.is_infinity() {
                    return p;
                }
            }
        }

        fn get_random_fp(rnd: &mut RAND) -> FP2 {
            let c = get_381bit_big(rnd);
            let d = get_381bit_big(rnd);
            FP2::new_bigs(c, d)
        }

        fn _get_random_fp<R: Rng + ?Sized>(rng: &mut R) -> Fq2 {
            Fq2::new(Fq::rand(rng), Fq::rand(rng))
        }

        fn serialize_fp(fq: &Fq2) -> Vec<u8> {
            let c1_bytes = G1Operations::serialize_fp(&fq.c1);
            let c0_bytes = G1Operations::serialize_fp(&fq.c0);
            vec![c1_bytes, c0_bytes].concat()
        }
    }

    macro_rules! impl_goperations {
        (
            $GOperations:ident,
            $ECP:ident,
            $FP:ident,
            $GConfig:ident,
            $GAffine:ident,
            $subgroup_check_g:ident,
            $serialize_g:ident,
            $add_p_y:ident,
            $serialize_uncompressed_g:ident,
            $bls12381_decompress:ident,
            $bls12381_sum:ident,
            $bls12381_multiexp:ident,
            $bls12381_map_fp_to_g:ident
        ) => {
            impl $GOperations {
                fn get_random_g_point(rnd: &mut RAND) -> $ECP {
                    let r: Big = Big::random(rnd);
                    let g: $ECP = $ECP::generator();

                    g.mul(&r)
                }

                fn _get_random_curve_point<R: Rng + ?Sized>(rng: &mut R) -> $GAffine {
                    loop {
                        let x = Self::_get_random_fp(rng);
                        let greatest = rng.gen();

                        if let Some(p) = $GAffine::get_point_from_x_unchecked(x, greatest) {
                            return p;
                        }
                    }
                }

                fn _get_random_g_point<R: Rng + ?Sized>(rng: &mut R) -> $GAffine {
                    let p = Self::_get_random_curve_point(rng);
                    p.clear_cofactor()
                }

                fn _get_random_not_g_curve_point<R: Rng + ?Sized>(rng: &mut R) -> $GAffine {
                    let mut p = Self::_get_random_curve_point(rng);
                    while p.is_in_correct_subgroup_assuming_on_curve() {
                        p = Self::_get_random_curve_point(rng);
                    }
                    p
                }

                fn get_random_not_g_curve_point(rnd: &mut RAND) -> $ECP {
                    let mut p = Self::get_random_curve_point(rnd);
                    while $subgroup_check_g(&p) {
                        p = Self::get_random_curve_point(rnd);
                    }

                    p
                }

                fn check_multipoint_sum<R: Rng + ?Sized>(n: usize, rng: &mut R) {
                    let mut res3 = $GAffine::identity();

                    let mut points: Vec<(u8, $GAffine)> = vec![];
                    for i in 0..n {
                        points.push((rng.gen_range(0..=1), Self::_get_random_curve_point(rng)));

                        let mut current_point = points[i].1.clone();
                        if points[i].0 == 1 {
                            current_point = current_point.neg();
                        }

                        res3 = res3.add(&current_point).into();
                    }

                    let res1 = Self::get_sum_many_points(&points);

                    points.shuffle(&mut thread_rng());
                    let res2 = Self::get_sum_many_points(&points);
                    assert_eq!(res1, res2);

                    assert_eq!(res1, Self::serialize_uncompressed_g(&res3).to_vec());
                }

                fn decompress_p(p2: Vec<$ECP>) -> Vec<u8> {
                    let mut p2s_vec: Vec<Vec<u8>> = vec![vec![]];
                    for i in 0..p2.len() {
                        p2s_vec.push($serialize_g(&p2[i]).to_vec());
                    }

                    run_bls12381_fn!($bls12381_decompress, p2s_vec)
                }

                fn _decompress_p(ps: Vec<$GAffine>) -> Vec<u8> {
                    let mut ps_vec: Vec<Vec<u8>> = vec![vec![]];
                    for i in 0..ps.len() {
                        ps_vec.push(Self::serialize_g(&ps[i]).to_vec());
                    }

                    run_bls12381_fn!($bls12381_decompress, ps_vec)
                }

                fn get_sum(p_sign: u8, p: &[u8], q_sign: u8, q: &[u8]) -> Vec<u8> {
                    let buffer = vec![vec![p_sign], p.to_vec(), vec![q_sign], q.to_vec()];
                    run_bls12381_fn!($bls12381_sum, buffer)
                }

                fn get_inverse(p: &[u8]) -> Vec<u8> {
                    let buffer = vec![vec![1], p.to_vec()];
                    run_bls12381_fn!($bls12381_sum, buffer)
                }

                fn get_sum_many_points(points: &Vec<(u8, $GAffine)>) -> Vec<u8> {
                    let mut buffer: Vec<Vec<u8>> = vec![];
                    for i in 0..points.len() {
                        buffer.push(vec![points[i].0]);
                        buffer.push(Self::serialize_uncompressed_g(&points[i].1).to_vec());
                    }
                    run_bls12381_fn!($bls12381_sum, buffer)
                }

                fn get_multiexp(points: &Vec<(Fr, $GAffine)>) -> Vec<u8> {
                    let mut buffer: Vec<Vec<u8>> = vec![];
                    for i in 0..points.len() {
                        buffer.push(Self::serialize_uncompressed_g(&points[i].1).to_vec());

                        let mut n_vec: [u8; 32] = [0u8; 32];
                        points[i].0.serialize_with_flags(n_vec.as_mut_slice(), EmptyFlags).unwrap();
                        buffer.push(n_vec.to_vec());
                    }

                    run_bls12381_fn!($bls12381_multiexp, buffer)
                }

                fn get_multiexp_small(points: &Vec<(u8, $GAffine)>) -> Vec<u8> {
                    let mut buffer: Vec<Vec<u8>> = vec![];
                    for i in 0..points.len() {
                        buffer.push(Self::serialize_uncompressed_g(&points[i].1).to_vec());
                        let mut n_vec: [u8; 32] = [0u8; 32];
                        n_vec[0] = points[i].0;
                        buffer.push(n_vec.to_vec());
                    }

                    run_bls12381_fn!($bls12381_multiexp, buffer)
                }

                fn get_multiexp_many_points(points: &Vec<(u8, $ECP)>) -> Vec<u8> {
                    let mut buffer: Vec<Vec<u8>> = vec![];
                    for i in 0..points.len() {
                        buffer.push($serialize_uncompressed_g(&points[i].1).to_vec());
                        if points[i].0 == 0 {
                            buffer.push(vec![vec![1], vec![0; 31]].concat());
                        } else {
                            buffer
                                .push(hex::decode(R_MINUS_1).unwrap().into_iter().rev().collect());
                        }
                    }

                    run_bls12381_fn!($bls12381_multiexp, buffer)
                }

                fn _get_multiexp_many_points(points: &Vec<(u8, $GAffine)>) -> Vec<u8> {
                    let mut buffer: Vec<Vec<u8>> = vec![];
                    for i in 0..points.len() {
                        buffer.push(Self::serialize_uncompressed_g(&points[i].1).to_vec());
                        if points[i].0 == 0 {
                            buffer.push(vec![vec![1], vec![0; 31]].concat());
                        } else {
                            buffer
                                .push(hex::decode(R_MINUS_1).unwrap().into_iter().rev().collect());
                        }
                    }

                    run_bls12381_fn!($bls12381_multiexp, buffer)
                }

                fn map_fp_to_g(fps: Vec<$FP>) -> Vec<u8> {
                    let mut fp_vec: Vec<Vec<u8>> = vec![];

                    for i in 0..fps.len() {
                        fp_vec.push(Self::serialize_fp(&fps[i]));
                    }

                    run_bls12381_fn!($bls12381_map_fp_to_g, fp_vec)
                }

                fn get_incorrect_points() -> Vec<Vec<u8>> {
                    let mut rnd = get_rnd();
                    let mut res: Vec<Vec<u8>> = vec![];

                    // Incorrect encoding of the point at infinity
                    let mut zero = get_zero(Self::POINT_LEN);
                    zero[Self::POINT_LEN - 1] = 1;
                    res.push(zero);

                    // Erroneous coding of field elements with an incorrect extra bit in the decompressed encoding.
                    let mut zero = vec![0u8; Self::POINT_LEN];
                    zero[0] = 192;
                    res.push(zero);

                    let p = Self::get_random_curve_point(&mut rnd);
                    let mut p_ser = $serialize_uncompressed_g(&p);
                    p_ser[0] |= 0x80;
                    res.push(p_ser.to_vec());

                    // Point not on the curve
                    let p = Self::get_random_curve_point(&mut rnd);
                    let mut p_ser = $serialize_uncompressed_g(&p);
                    p_ser[$GOperations::POINT_LEN - 1] ^= 0x01;
                    res.push(p_ser.to_vec());

                    //Erroneous coding of field elements, resulting in a correct point on the curve if only the suffix is considered.
                    let p = Self::get_random_curve_point(&mut rnd);
                    let mut p_ser = $serialize_uncompressed_g(&p);
                    p_ser[0] ^= 0x20;
                    res.push(p_ser.to_vec());

                    let p = Self::get_random_curve_point(&mut rnd);
                    let p_ser = $add_p_y(&p).to_vec();
                    res.push(p_ser);

                    res
                }

                fn map_to_curve_g(fp: $FP) -> $GAffine {
                    let wbmap =
                        WBMap::<<ark_bls12_381::Config as Bls12Config>::$GConfig>::new().unwrap();
                    let res = wbmap.map_to_curve(fp).unwrap();
                    if res.infinity {
                        return $GAffine::identity();
                    }

                    $GAffine::new_unchecked(res.x, res.y)
                }

                fn serialize_uncompressed_g(p: &$GAffine) -> Vec<u8> {
                    let mut serialized = vec![0u8; Self::POINT_LEN];
                    p.serialize_with_mode(serialized.as_mut_slice(), ark_serialize::Compress::No)
                        .unwrap();

                    serialized
                }

                fn serialize_g(p: &$GAffine) -> Vec<u8> {
                    let mut serialized = vec![0u8; Self::POINT_LEN/2];
                    p.serialize_with_mode(serialized.as_mut_slice(), ark_serialize::Compress::Yes)
                        .unwrap();

                    serialized
                }

                fn deserialize_g(p: Vec<u8>) -> $GAffine {
                    $GAffine::deserialize_with_mode(p.as_slice(), ark_serialize::Compress::No, ark_serialize::Validate::No).unwrap()
                }
            }
        };
    }

    impl_goperations!(
        G1Operations,
        ECP,
        Fq,
        G1Config,
        G1Affine,
        subgroup_check_g1,
        serialize_g1,
        add_p_y,
        serialize_uncompressed_g1,
        bls12381_p1_decompress,
        bls12381_p1_sum,
        bls12381_p1_multiexp,
        bls12381_map_fp_to_g1
    );
    impl_goperations!(
        G2Operations,
        ECP2,
        Fq2,
        G2Config,
        G2Affine,
        subgroup_check_g2,
        serialize_g2,
        add2_p_y,
        serialize_uncompressed_g2,
        bls12381_p2_decompress,
        bls12381_p2_sum,
        bls12381_p2_multiexp,
        bls12381_map_fp2_to_g2
    );

    fn get_rnd() -> RAND {
        let mut rnd: RAND = RAND::new();
        rnd.clean();
        let mut raw: [u8; 100] = [0; 100];
        for i in 0..100 {
            raw[i] = i as u8
        }
        rnd.seed(100, &raw);
        rnd
    }

    fn get_zero(point_len: usize) -> Vec<u8> {
        let mut zero1 = vec![0; point_len];
        zero1[0] |= 0x40;
        zero1
    }

    fn get_n(i: usize, max_n: usize) -> usize {
        return if i == 0 { max_n } else { (thread_rng().next_u32() as usize) % max_n };
    }

    fn pairing_check(p1s: Vec<ECP>, p2s: Vec<ECP2>) -> u64 {
        let mut logic_builder = VMLogicBuilder::default();
        let mut logic = logic_builder.build();

        let mut buffer: Vec<Vec<u8>> = vec![];
        for i in 0..p1s.len() {
            buffer.push(serialize_uncompressed_g1(&p1s[i]).to_vec());
            buffer.push(serialize_uncompressed_g2(&p2s[i]).to_vec());
        }

        let input = logic.internal_mem_write(&buffer.concat().as_slice());
        let res = logic.bls12381_pairing_check(input.len, input.ptr).unwrap();
        return res;
    }

    fn pairing_check_vec(p1: Vec<u8>, p2: Vec<u8>) -> u64 {
        let mut logic_builder = VMLogicBuilder::default();
        let mut logic = logic_builder.build();

        let buffer: Vec<Vec<u8>> = vec![p1, p2];

        let input = logic.internal_mem_write(&buffer.concat().as_slice());
        let res = logic.bls12381_pairing_check(input.len, input.ptr).unwrap();
        return res;
    }

    macro_rules! test_bls12381_sum {
        (
            $GOp:ident,
            $GAffine:ident,
            $bls12381_sum:ident,
            $check_sum:ident,
            $test_bls12381_sum_edge_cases:ident,
            $test_bls12381_sum:ident,
            $test_bls12381_sum_not_g_points:ident,
            $test_bls12381_sum_inverse:ident,
            $test_bls12381_sum_many_points:ident,
            $test_bls12381_crosscheck_sum_and_multiexp:ident,
            $test_bls12381_sum_incorrect_input:ident
        ) => {
            #[test]
            fn $test_bls12381_sum_edge_cases() {
                // 0 + 0
                let zero = get_zero($GOp::POINT_LEN);
                assert_eq!(zero.to_vec(), $GOp::get_sum(0, &zero, 0, &zero));

                // 0 + P = P + 0 = P
                let mut rng = test_rng();
                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_g_point(&mut rng);
                    let p_ser = $GOp::serialize_uncompressed_g(&p);
                    assert_eq!(p_ser.to_vec(), $GOp::get_sum(0, &zero, 0, &p_ser));
                    assert_eq!(p_ser.to_vec(), $GOp::get_sum(0, &p_ser, 0, &zero));
                }

                // P + P
                // P + (-P) = (-P) + P =  0
                // P + (-(P + P))
                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_curve_point(&mut rng);
                    let p_ser = $GOp::serialize_uncompressed_g(&p);

                    let pmul2 = p.mul(Fr::from(2));
                    let pmul2_ser = $GOp::serialize_uncompressed_g(&pmul2.into_affine());
                    assert_eq!(pmul2_ser.to_vec(), $GOp::get_sum(0, &p_ser, 0, &p_ser));

                    let pneg = p.neg();
                    let p_neg_ser = $GOp::serialize_uncompressed_g(&pneg);

                    assert_eq!(zero.to_vec(), $GOp::get_sum(0, &p_neg_ser, 0, &p_ser));
                    assert_eq!(zero.to_vec(), $GOp::get_sum(0, &p_ser, 0, &p_neg_ser));

                    let pmul2neg = pmul2.neg();
                    let pmul2_neg = $GOp::serialize_uncompressed_g(&pmul2neg.into_affine());
                    assert_eq!(p_neg_ser.to_vec(), $GOp::get_sum(0, &p_ser, 0, &pmul2_neg));
                }
            }

            fn $check_sum(p: $GAffine, q: $GAffine) {
                let p_ser = $GOp::serialize_uncompressed_g(&p);
                let q_ser = $GOp::serialize_uncompressed_g(&q);

                // P + Q = Q + P
                let got1 = $GOp::get_sum(0, &p_ser, 0, &q_ser);
                let got2 = $GOp::get_sum(0, &q_ser, 0, &p_ser);
                assert_eq!(got1, got2);

                // compare with library results
                let psum = p.add(&q);
                let library_sum = $GOp::serialize_uncompressed_g(&psum.into_affine());

                assert_eq!(library_sum.to_vec(), got1);

                let p_inv = $GOp::get_inverse(&library_sum);
                let pneg = psum.neg();
                let p_neg_ser = $GOp::serialize_uncompressed_g(&pneg.into_affine());

                assert_eq!(p_neg_ser.to_vec(), p_inv);
            }

            #[test]
            fn $test_bls12381_sum() {
                let mut rng = test_rng();

                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_curve_point(&mut rng);
                    let q = $GOp::_get_random_curve_point(&mut rng);

                    $check_sum(p, q);
                }

                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_g_point(&mut rng);
                    let q = $GOp::_get_random_g_point(&mut rng);

                    let p_ser = $GOp::serialize_uncompressed_g(&p);
                    let q_ser = $GOp::serialize_uncompressed_g(&q);

                    let got1 = $GOp::get_sum(0, &p_ser, 0, &q_ser);

                    let result_point = $GOp::deserialize_g(got1);
                    assert!(result_point.is_in_correct_subgroup_assuming_on_curve());
                }
            }

            #[test]
            fn $test_bls12381_sum_not_g_points() {
                let mut rng = test_rng();

                //points not from G
                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_not_g_curve_point(&mut rng);
                    let q = $GOp::_get_random_not_g_curve_point(&mut rng);

                    $check_sum(p, q);
                }
            }

            #[test]
            fn $test_bls12381_sum_inverse() {
                let mut rng = test_rng();

                let zero = get_zero($GOp::POINT_LEN);
                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_curve_point(&mut rng);
                    let p_ser = $GOp::serialize_uncompressed_g(&p);

                    // P - P = - P + P = 0
                    let got1 = $GOp::get_sum(1, &p_ser, 0, &p_ser);
                    let got2 = $GOp::get_sum(0, &p_ser, 1, &p_ser);
                    assert_eq!(got1, got2);
                    assert_eq!(got1, zero.to_vec());

                    // -(-P)
                    let p_inv = $GOp::get_inverse(&p_ser);
                    let p_inv_inv = $GOp::get_inverse(p_inv.as_slice());

                    assert_eq!(p_ser.to_vec(), p_inv_inv);
                }

                // P in G => -P in G
                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_g_point(&mut rng);
                    let p_ser = $GOp::serialize_uncompressed_g(&p);

                    let p_inv = $GOp::get_inverse(&p_ser);

                    let result_point = $GOp::deserialize_g(p_inv);
                    assert!(result_point.is_in_correct_subgroup_assuming_on_curve());
                }

                // -0
                let zero_inv = $GOp::get_inverse(&zero);
                assert_eq!(zero.to_vec(), zero_inv);
            }

            #[test]
            fn $test_bls12381_sum_many_points() {
                let mut rng = test_rng();

                let zero = get_zero($GOp::POINT_LEN);
                //empty input
                let res = $GOp::get_sum_many_points(&vec![]);
                assert_eq!(zero.to_vec(), res);

                for i in 0..TESTS_ITERATIONS {
                    $GOp::check_multipoint_sum(get_n(i, $GOp::MAX_N_SUM), &mut rng);
                }
                $GOp::check_multipoint_sum(1, &mut rng);

                for i in 0..TESTS_ITERATIONS {
                    let n = get_n(i, $GOp::MAX_N_SUM);
                    let mut points: Vec<(u8, $GAffine)> = vec![];
                    for _ in 0..n {
                        points.push((rng.gen_range(0..=1), $GOp::_get_random_g_point(&mut rng)));
                    }

                    let res1 = $GOp::get_sum_many_points(&points);
                    let sum = $GOp::deserialize_g(res1);

                    assert!(sum.is_in_correct_subgroup_assuming_on_curve());
                }
            }

            #[test]
            fn $test_bls12381_crosscheck_sum_and_multiexp() {
                let mut rng = test_rng();

                for i in 0..TESTS_ITERATIONS {
                    let n = get_n(i, $GOp::MAX_N_MULTIEXP);

                    let mut points: Vec<(u8, $GAffine)> = vec![];
                    for _ in 0..n {
                        points.push((rng.gen_range(0..=1), $GOp::_get_random_g_point(&mut rng)));
                    }

                    let res1 = $GOp::get_sum_many_points(&points);
                    let res2 = $GOp::_get_multiexp_many_points(&points);
                    assert_eq!(res1, res2);
                }
            }

            #[test]
            fn $test_bls12381_sum_incorrect_input() {
                let mut test_vecs: Vec<Vec<Vec<u8>>> = $GOp::get_incorrect_points()
                    .into_iter()
                    .map(|test| vec![vec![0u8], test])
                    .collect();

                // Incorrect sign encoding
                test_vecs.push(vec![vec![2u8], get_zero($GOp::POINT_LEN)]);

                for i in 0..test_vecs.len() {
                    run_bls12381_fn!($bls12381_sum, test_vecs[i], 1);
                }
            }
        };
    }

    test_bls12381_sum!(
        G1Operations,
        G1Affine,
        bls12381_p1_sum,
        check_sum_p1,
        test_bls12381_p1_sum_edge_cases,
        test_bls12381_p1_sum,
        test_bls12381_p1_sum_not_g1_points,
        test_bls12381_p1_sum_inverse,
        test_bls12381_p1_sum_many_points,
        test_bls12381_p1_crosscheck_sum_and_multiexp,
        test_bls12381_p1_sum_incorrect_input
    );
    test_bls12381_sum!(
        G2Operations,
        G2Affine,
        bls12381_p2_sum,
        check_sum_p2,
        test_bls12381_p2_sum_edge_cases,
        test_bls12381_p2_sum,
        test_bls12381_p2_sum_not_g2_points,
        test_bls12381_p2_sum_inverse,
        test_bls12381_p2_sum_many_points,
        test_bls12381_p2_crosscheck_sum_and_multiexp,
        test_bls12381_p2_sum_incorrect_input
    );

    macro_rules! test_bls12381_memory_limit {
        (
            $namespace_name:ident,
            $INPUT_SIZE:expr,
            $MAX_N:expr,
            $run_bls_fn:ident
        ) => {
            mod $namespace_name {
                use crate::logic::tests::bls12381::tests::$run_bls_fn;
                use crate::logic::tests::vm_logic_builder::VMLogicBuilder;

                // Input is beyond memory bounds.
                #[test]
                #[should_panic]
                fn test_bls12381_too_big_input() {
                    let mut logic_builder = VMLogicBuilder::default();
                    let mut logic = logic_builder.build();

                    let buffer = vec![0u8; $INPUT_SIZE * $MAX_N];

                    let input = logic.internal_mem_write(buffer.as_slice());
                    $run_bls_fn(input, &mut logic);
                }

                #[test]
                #[should_panic]
                fn test_bls12381_incorrect_length() {
                    let mut logic_builder = VMLogicBuilder::default();
                    let mut logic = logic_builder.build();

                    let buffer = vec![0u8; $INPUT_SIZE - 1];

                    let input = logic.internal_mem_write(buffer.as_slice());
                    $run_bls_fn(input, &mut logic);
                }
            }
        };
    }

    test_bls12381_memory_limit!(memory_limit_p1_sum, 97, 676, sum_g1_return_value);
    test_bls12381_memory_limit!(memory_limit_p2_sum, 193, 340, sum_g2_return_value);
    test_bls12381_memory_limit!(memory_limit_p1_multiexp, 128, 600, multiexp_g1_return_value);
    test_bls12381_memory_limit!(memory_limit_p2_multiexp, 224, 300, multiexp_g2_return_value);
    test_bls12381_memory_limit!(memory_limit_map_fp_to_g1, 48, 1500, map_fp_to_g1_return_value);
    test_bls12381_memory_limit!(memory_limit_map_fp2_to_g2, 96, 700, map_fp2tog2_return_value);
    test_bls12381_memory_limit!(memory_limit_p1_decompress, 48, 1500, decompress_g1_return_value);
    test_bls12381_memory_limit!(memory_limit_p2_decompress, 96, 700, decompress_g2_return_value);
    test_bls12381_memory_limit!(memory_limit_pairing_check, 288, 500, run_pairing_check_raw);

    macro_rules! test_bls12381_multiexp {
        (
            $GOp:ident,
            $GAffine:ident,
            $bls12381_multiexp:ident,
            $bls12381_sum:ident,
            $test_bls12381_multiexp_mul:ident,
            $test_bls12381_multiexp_many_points: ident,
            $test_bls12381_multiexp_incorrect_input: ident,
            $test_bls12381_multiexp_invariants_checks: ident,
            $test_bls12381_error_encoding: ident
        ) => {
            #[test]
            fn $test_bls12381_multiexp_mul() {
                let mut rng = test_rng();

                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_curve_point(&mut rng);
                    let n = rng.gen_range(0..200) as usize;

                    let points: Vec<(u8, $GAffine)> = vec![(0, p.clone()); n];
                    let res1 = $GOp::get_sum_many_points(&points);
                    let res2 = $GOp::get_multiexp_small(&vec![(n as u8, p.clone())]);

                    assert_eq!(res1, res2);
                    let res3 = p.mul(Fr::from(n as u64));
                    assert_eq!(res1, $GOp::serialize_uncompressed_g(&res3.into()));
                }

                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_curve_point(&mut rng);
                    let distr = ark_std::rand::distributions::Standard;
                    let n: Fr = distr.sample(&mut rng);

                    let res1 = $GOp::get_multiexp(&vec![(n.clone(), p.clone())]);
                    let res2 = p.mul(&n);

                    assert_eq!(res1, $GOp::serialize_uncompressed_g(&res2.into()));
                }
            }

            #[test]
            fn $test_bls12381_multiexp_many_points() {
                let mut rng = test_rng();

                for i in 0..TESTS_ITERATIONS {
                    let n = get_n(i, $GOp::MAX_N_MULTIEXP);
                    let mut res2 = $GAffine::identity();

                    let mut points: Vec<(Fr, $GAffine)> = vec![];
                    for i in 0..n {
                        let distr = ark_std::rand::distributions::Standard;
                        let scalar: Fr = distr.sample(&mut rng);

                        points.push((scalar, $GOp::_get_random_curve_point(&mut rng)));
                        res2 = res2.add(&points[i].1.mul(&points[i].0)).into();
                    }

                    let res1 = $GOp::get_multiexp(&points);
                    assert_eq!(res1, $GOp::serialize_uncompressed_g(&res2.into()));
                }
            }

            #[test]
            fn $test_bls12381_multiexp_incorrect_input() {
                let zero_scalar = vec![0u8; 32];

                let test_vecs: Vec<Vec<Vec<u8>>> = $GOp::get_incorrect_points()
                    .into_iter()
                    .map(|test| vec![test, zero_scalar.clone()])
                    .collect();

                for i in 0..test_vecs.len() {
                    run_bls12381_fn!($bls12381_multiexp, test_vecs[i], 1);
                }
            }

            #[test]
            fn $test_bls12381_multiexp_invariants_checks() {
                let zero1 = get_zero($GOp::POINT_LEN);

                let mut rng = test_rng();
                let r = Fr::from_str(R).unwrap();

                for _ in 0..TESTS_ITERATIONS {
                    let p = $GOp::_get_random_g_point(&mut rng);

                    // group_order * P = 0
                    let res = $GOp::get_multiexp(&vec![(r.clone(), p.clone())]);
                    assert_eq!(res.as_slice(), zero1);

                    let distr = ark_std::rand::distributions::Standard;
                    let mut scalar: Fr = distr.sample(&mut rng);

                    // (scalar + group_order) * P = scalar * P
                    let res1 = $GOp::get_multiexp(&vec![(scalar.clone(), p.clone())]);
                    scalar = scalar.add(&r);
                    let res2 = $GOp::get_multiexp(&vec![(scalar.clone(), p.clone())]);
                    assert_eq!(res1, res2);

                    // P + P + ... + P = N * P
                    let n = rng.gen_range(0..200);
                    let res1 = $GOp::get_multiexp(&vec![(Fr::one(), p.clone()); n as usize]);
                    let res2 = $GOp::get_multiexp(&vec![(Fr::from(n as u8), p.clone())]);
                    assert_eq!(res1, res2);

                    // 0 * P = 0
                    let res1 = $GOp::get_multiexp(&vec![(Fr::zero(), p.clone())]);
                    assert_eq!(res1, zero1);

                    // 1 * P = P
                    let res1 = $GOp::get_multiexp(&vec![(Fr::one(), p.clone())]);
                    assert_eq!(res1, $GOp::serialize_uncompressed_g(&p));
                }
            }
        };
    }

    test_bls12381_multiexp!(
        G1Operations,
        G1Affine,
        bls12381_p1_multiexp,
        bls12381_p1_sum,
        test_bls12381_p1_multiexp_mul,
        test_bls12381_p1_multiexp_many_points,
        test_bls12381_p1_multiexp_incorrect_input,
        test_bls12381_p1_multiexp_invariants_checks,
        test_bls12381_error_g1_encoding
    );
    test_bls12381_multiexp!(
        G2Operations,
        G2Affine,
        bls12381_p2_multiexp,
        bls12381_p2_sum,
        test_bls12381_p2_multiexp_mul,
        test_bls12381_p2_multiexp_many_points,
        test_bls12381_p2_multiexp_incorrect_input,
        test_bls12381_p2_multiexp_invariants_checks,
        test_bls12381_error_g2_encoding
    );

    fn add_p_y(point: &ECP) -> [u8; 96] {
        let mut ybig = point.gety();
        ybig.add(&Big::from_string(P.to_string()));
        let mut p_ser = serialize_uncompressed_g1(&point);
        ybig.to_byte_array(&mut p_ser[0..96], 48);

        p_ser
    }

    fn add2_p_y(point: &ECP2) -> [u8; 192] {
        let mut yabig = point.gety().geta();
        yabig.add(&Big::from_string(P.to_string()));
        let mut p_ser = serialize_uncompressed_g2(&point);
        yabig.to_byte_array(&mut p_ser[0..192], 96 + 48);

        p_ser
    }

    macro_rules! test_bls12381_map_fp_to_g {
        (
            $GOp:ident,
            $map_to_curve_g:ident,
            $FP:ident,
            $check_map_fp:ident,
            $test_bls12381_map_fp_to_g:ident,
            $test_bls12381_map_fp_to_g_many_points:ident
        ) => {
            fn $check_map_fp(fp: $FP) {
                let res1 = $GOp::map_fp_to_g(vec![fp.clone()]);

                let mut res2 = $GOp::map_to_curve_g(fp);
                res2 = res2.clear_cofactor();

                assert_eq!(res1, $GOp::serialize_uncompressed_g(&res2));
            }

            #[test]
            fn $test_bls12381_map_fp_to_g() {
                let mut rng = test_rng();

                for _ in 0..TESTS_ITERATIONS {
                    $check_map_fp($GOp::_get_random_fp(&mut rng));
                }
            }

            #[test]
            fn $test_bls12381_map_fp_to_g_many_points() {
                let mut rng = test_rng();

                for i in 0..TESTS_ITERATIONS {
                    let n = get_n(i, $GOp::MAX_N_MAP);

                    let mut fps: Vec<$FP> = vec![];
                    let mut res2_mul: Vec<u8> = vec![];
                    for i in 0..n {
                        fps.push($GOp::_get_random_fp(&mut rng));

                        let mut res2 = $GOp::map_to_curve_g(fps[i].clone());
                        res2 = res2.clear_cofactor();

                        res2_mul.append(&mut $GOp::serialize_uncompressed_g(&res2));
                    }

                    let res1 = $GOp::map_fp_to_g(fps);
                    assert_eq!(res1, res2_mul);
                }
            }
        };
    }

    test_bls12381_map_fp_to_g!(
        G1Operations,
        map_to_curve_g1,
        Fq,
        check_map_fp,
        test_bls12381_map_fp_to_g1,
        test_bls12381_map_fp_to_g1_many_points
    );

    test_bls12381_map_fp_to_g!(
        G2Operations,
        map_to_curve_g2,
        Fq2,
        check_map_fp2,
        test_bls12381_map_fp2_to_g2,
        test_bls12381_map_fp2_to_g2_many_points
    );

    #[test]
    fn test_bls12381_map_fp_to_g1_edge_cases() {
        check_map_fp(Fq::ZERO);
        check_map_fp(Fq::from_str(P_MINUS_1).unwrap());
    }

    #[test]
    fn test_bls12381_map_fp_to_g1_incorrect_input() {
        let p = hex::decode(P.to_string()).unwrap();
        run_bls12381_fn!(bls12381_map_fp_to_g1, vec![p], 1);
    }

    #[test]
    fn test_bls12381_map_fp2_to_g2_incorrect_input() {
        let p = hex::decode(P.to_string()).unwrap();
        run_bls12381_fn!(bls12381_map_fp2_to_g2, vec![p.clone(), vec![0u8; 48]], 1);
        run_bls12381_fn!(bls12381_map_fp2_to_g2, vec![vec![0u8; 48], p.clone()], 1);
    }

    macro_rules! test_bls12381_decompress {
        (
            $GOp:ident,
            $GAffine:ident,
            $serialize_uncompressed_g:ident,
            $serialize_g:ident,
            $POINT_LEN:expr,
            $ECP:ident,
            $bls12381_decompress:ident,
            $add_p:ident,
            $test_bls12381_decompress:ident,
            $test_bls12381_decompress_many_points:ident,
            $test_bls12381_decompress_incorrect_input:ident
        ) => {
            #[test]
            fn $test_bls12381_decompress() {
                let mut rng = test_rng();

                for _ in 0..TESTS_ITERATIONS {
                    let p1 = $GOp::_get_random_g_point(&mut rng);
                    let res1 = $GOp::_decompress_p(vec![p1.clone()]);

                    assert_eq!(res1, $GOp::serialize_uncompressed_g(&p1));

                    let p1_neg = p1.mul(&Fr::from(-1));
                    let res1_neg = $GOp::_decompress_p(vec![p1_neg.clone().into()]);

                    assert_eq!(res1[0..$POINT_LEN], res1_neg[0..$POINT_LEN]);
                    assert_ne!(res1[$POINT_LEN..], res1_neg[$POINT_LEN..]);
                    assert_eq!(res1_neg, $GOp::serialize_uncompressed_g(&p1_neg.into()));
                }

                let zero1 = $GAffine::identity();
                let res1 = $GOp::_decompress_p(vec![zero1.clone()]);

                assert_eq!(res1, $GOp::serialize_uncompressed_g(&zero1));
            }

            #[test]
            fn $test_bls12381_decompress_many_points() {
                let mut rng = test_rng();

                for i in 0..TESTS_ITERATIONS {
                    let n = get_n(i, $GOp::MAX_N_DECOMPRESS);

                    let mut p1s: Vec<$GAffine> = vec![];
                    let mut res2: Vec<u8> = vec![];
                    for i in 0..n {
                        p1s.push($GOp::_get_random_curve_point(&mut rng));
                        res2.append(&mut $GOp::serialize_uncompressed_g(&p1s[i]).to_vec());
                    }
                    let res1 = $GOp::_decompress_p(p1s.clone());
                    assert_eq!(res1, res2);

                    let mut p1s: Vec<$GAffine> = vec![];
                    let mut res2: Vec<u8> = vec![];
                    for i in 0..n {
                        p1s.push($GOp::_get_random_g_point(&mut rng));
                        res2.append(&mut $GOp::serialize_uncompressed_g(&p1s[i]).to_vec());
                    }
                    let res1 = $GOp::_decompress_p(p1s.clone());
                    assert_eq!(res1, res2);
                }
            }

            #[test]
            fn $test_bls12381_decompress_incorrect_input() {
                let mut rnd = get_rnd();

                // Incorrect encoding of the point at infinity
                let mut zero = vec![0u8; $POINT_LEN];
                zero[0] = 0x80 | 0x40;
                zero[$POINT_LEN - 1] = 1;
                run_bls12381_fn!($bls12381_decompress, vec![zero], 1);

                // Erroneous coding of field elements with an incorrect extra bit in the decompressed encoding.
                let mut zero = vec![0u8; $POINT_LEN];
                zero[0] = 0x40;
                run_bls12381_fn!($bls12381_decompress, vec![zero], 1);

                let p = $GOp::get_random_curve_point(&mut rnd);
                let mut p_ser = $serialize_g(&p);
                p_ser[0] ^= 0x80;
                run_bls12381_fn!($bls12381_decompress, vec![p_ser], 1);

                //Point with a coordinate larger than 'p'.
                let p = $GOp::get_random_curve_point(&mut rnd);
                run_bls12381_fn!($bls12381_decompress, vec![$add_p(&p)], 1);
            }
        };
    }

    test_bls12381_decompress!(
        G1Operations,
        G1Affine,
        serialize_uncompressed_g1,
        serialize_g1,
        48,
        ECP,
        bls12381_p1_decompress,
        add_p_x,
        test_bls12381_p1_decompress,
        test_bls12381_p1_decompress_many_points,
        test_bls12381_p1_decompress_incorrect_input
    );

    test_bls12381_decompress!(
        G2Operations,
        G2Affine,
        serialize_uncompressed_g2,
        serialize_g2,
        96,
        ECP2,
        bls12381_p2_decompress,
        add2_p_x,
        test_bls12381_p2_decompress,
        test_bls12381_p2_decompress_many_points,
        test_bls12381_p2_decompress_incorrect_input
    );

    fn add_p_x(point: &ECP) -> [u8; 48] {
        let mut xbig = point.getx();
        xbig.add(&Big::from_string(P.to_string()));
        let mut p_ser = serialize_g1(&point);
        xbig.to_byte_array(&mut p_ser[0..48], 0);
        p_ser[0] |= 0x80;

        p_ser
    }

    fn add2_p_x(point: &ECP2) -> [u8; 96] {
        let mut xabig = point.getx().geta();
        xabig.add(&Big::from_string(P.to_string()));
        let mut p_ser = serialize_g2(&point);
        xabig.to_byte_array(&mut p_ser[0..96], 48);

        p_ser
    }

    #[test]
    fn test_bls12381_pairing_check_one_point() {
        let mut rnd = get_rnd();

        for _ in 0..TESTS_ITERATIONS {
            let p1 = G1Operations::get_random_g_point(&mut rnd);
            let p2 = G2Operations::get_random_g_point(&mut rnd);

            let zero1 = ECP::new();
            let zero2 = ECP2::new();

            let mut r = pair::initmp();
            pair::another(&mut r, &zero2, &p1);
            let mut v = pair::miller(&r);

            v = pair::fexp(&v);
            assert!(v.is_unity());

            assert_eq!(pairing_check(vec![zero1.clone()], vec![zero2.clone()]), 0);
            assert_eq!(pairing_check(vec![zero1.clone()], vec![p2.clone()]), 0);
            assert_eq!(pairing_check(vec![p1.clone()], vec![zero2.clone()]), 0);
            assert_eq!(pairing_check(vec![p1.clone()], vec![p2.clone()]), 2);
        }
    }

    #[test]
    fn test_bls12381_pairing_check_two_points() {
        let mut rnd = get_rnd();

        for _ in 0..TESTS_ITERATIONS {
            let p1 = G1Operations::get_random_g_point(&mut rnd);
            let p2 = G2Operations::get_random_g_point(&mut rnd);

            let p1_neg = p1.mul(&Big::new_int(-1));
            let p2_neg = p2.mul(&Big::new_int(-1));

            assert_eq!(
                pairing_check(vec![p1.clone(), p1_neg.clone()], vec![p2.clone(), p2.clone()]),
                0
            );
            assert_eq!(
                pairing_check(vec![p1.clone(), p1.clone()], vec![p2.clone(), p2_neg.clone()]),
                0
            );
            assert_eq!(
                pairing_check(vec![p1.clone(), p1.clone()], vec![p2.clone(), p2.clone()]),
                2
            );

            let mut s1 = Big::random(&mut rnd);
            s1.mod2m(32 * 8);

            let mut s2 = Big::random(&mut rnd);
            s2.mod2m(32 * 8);

            assert_eq!(
                pairing_check(vec![p1.mul(&s1), p1_neg.mul(&s2)], vec![p2.mul(&s2), p2.mul(&s1)]),
                0
            );
            assert_eq!(
                pairing_check(vec![p1.mul(&s1), p1.mul(&s2)], vec![p2.mul(&s2), p2_neg.mul(&s1)]),
                0
            );
            assert_eq!(
                pairing_check(
                    vec![p1.mul(&s1), p1.mul(&s2)],
                    vec![p2_neg.mul(&s2), p2_neg.mul(&s1)]
                ),
                2
            );
        }
    }

    #[test]
    fn test_bls12381_pairing_check_many_points() {
        let mut rnd = get_rnd();

        let r = Big::from_string(R.to_string());
        for i in 0..TESTS_ITERATIONS {
            let n = get_n(i, MAX_N_PAIRING);

            let mut scalars_1: Vec<Big> = vec![];
            let mut scalars_2: Vec<Big> = vec![];

            let g1: ECP = ECP::generator();
            let g2: ECP2 = ECP2::generator();

            let mut g1s: Vec<ECP> = vec![];
            let mut g2s: Vec<ECP2> = vec![];

            let mut scalar_res = Big::new();

            for i in 0..n {
                scalars_1.push(Big::random(&mut rnd));
                scalars_2.push(Big::random(&mut rnd));

                scalars_1[i].rmod(&r);
                scalars_2[i].rmod(&r);

                scalar_res.add(&Big::smul(&scalars_1[i], &scalars_2[i]));
                scalar_res.rmod(&r);

                g1s.push(g1.mul(&scalars_1[i]));
                g2s.push(g2.mul(&scalars_2[i]));
            }

            if !scalar_res.is_zilch() {
                assert_eq!(pairing_check(g1s.clone(), g2s.clone()), 2);
            } else {
                assert_eq!(pairing_check(g1s.clone(), g2s.clone()), 0);
            }

            for i in 0..n {
                let mut p2 = g2.mul(&scalars_1[i]);
                p2.neg();

                g1s.push(g1.mul(&scalars_2[i]));
                g2s.push(p2);
            }

            assert_eq!(pairing_check(g1s, g2s), 0);
        }
    }

    #[test]
    fn test_bls12381_pairing_incorrect_input_point() {
        let mut rnd = get_rnd();

        let p1_not_from_g1 = G1Operations::get_random_not_g_curve_point(&mut rnd);
        let p2 = G2Operations::get_random_g_point(&mut rnd);

        let p1 = G1Operations::get_random_g_point(&mut rnd);
        let p2_not_from_g2 = G2Operations::get_random_not_g_curve_point(&mut rnd);

        assert_eq!(pairing_check(vec![p1_not_from_g1.clone()], vec![p2.clone()]), 1);
        assert_eq!(pairing_check(vec![p1.clone()], vec![p2_not_from_g2.clone()]), 1);

        let p1_ser = serialize_uncompressed_g1(&p1).to_vec();
        let p2_ser = serialize_uncompressed_g2(&p2).to_vec();
        let test_vecs: Vec<Vec<u8>> = G1Operations::get_incorrect_points();
        for i in 0..test_vecs.len() {
            assert_eq!(pairing_check_vec(test_vecs[i].clone(), p2_ser.clone()), 1);
        }

        let test_vecs: Vec<Vec<u8>> = G2Operations::get_incorrect_points();
        for i in 0..test_vecs.len() {
            assert_eq!(pairing_check_vec(p1_ser.clone(), test_vecs[i].clone()), 1);
        }

        // not G1 point
        let p = G1Operations::get_random_not_g_curve_point(&mut rnd);
        let p_ser = serialize_uncompressed_g1(&p);
        assert_eq!(pairing_check_vec(p_ser.to_vec(), serialize_uncompressed_g2(&p2).to_vec()), 1);

        // not G2 point
        let p = G2Operations::get_random_not_g_curve_point(&mut rnd);
        let p_ser = serialize_uncompressed_g2(&p);

        assert_eq!(pairing_check_vec(serialize_uncompressed_g1(&p1).to_vec(), p_ser.to_vec()), 1);
    }

    #[test]
    fn test_bls12381_empty_input() {
        assert_eq!(get_zero(96), G1Operations::get_multiexp_many_points(&vec![]));
        assert_eq!(get_zero(192), G2Operations::get_multiexp_many_points(&vec![]));
        assert_eq!(G1Operations::map_fp_to_g(vec![]).len(), 0);
        assert_eq!(G2Operations::map_fp_to_g(vec![]).len(), 0);
        assert_eq!(pairing_check(vec![], vec![]), 0);
        assert_eq!(G1Operations::decompress_p(vec![]).len(), 0);
        assert_eq!(G2Operations::decompress_p(vec![]).len(), 0);
    }

    // EIP-2537 tests
    macro_rules! eip2537_tests {
        (
            $file_path:expr,
            $test_name:ident,
            $item_size:expr,
            $transform_input:ident,
            $run_bls_fn:ident,
            $check_res:ident
        ) => {
            #[test]
            fn $test_name() {
                let input_csv = fs::read($file_path).unwrap();
                let mut reader = csv::Reader::from_reader(input_csv.as_slice());
                for record in reader.records() {
                    let record = record.unwrap();

                    let mut logic_builder = VMLogicBuilder::default();
                    let mut logic = logic_builder.build();

                    let bytes_input = hex::decode(&record[0]).unwrap();
                    let k = bytes_input.len() / $item_size;
                    let mut bytes_input_fix: Vec<Vec<u8>> = vec![];
                    for i in 0..k {
                        bytes_input_fix.push($transform_input(
                            bytes_input[i * $item_size..(i + 1) * $item_size].to_vec(),
                        ));
                    }

                    let input = logic.internal_mem_write(&bytes_input_fix.concat());
                    let res = $run_bls_fn(input, &mut logic);
                    $check_res(&record[1], res);
                }
            }
        };
    }

    fn fix_eip2537_pairing_input(input: Vec<u8>) -> Vec<u8> {
        vec![
            fix_eip2537_g1(input[..128].to_vec()).to_vec(),
            fix_eip2537_g2(input[128..].to_vec()).to_vec(),
        ]
        .concat()
    }

    fn fix_eip2537_fp(fp: Vec<u8>) -> Vec<u8> {
        fp[16..].to_vec()
    }

    fn fix_eip2537_fp2(fp2: Vec<u8>) -> Vec<u8> {
        vec![fp2[64 + 16..].to_vec(), fp2[16..64].to_vec()].concat()
    }

    macro_rules! fix_eip2537_input {
        ($namespace_name:ident, $fix_eip2537_fp:ident) => {
            mod $namespace_name {
                use crate::logic::tests::bls12381::tests::$fix_eip2537_fp;

                pub fn fix_eip2537_g(g: Vec<u8>) -> Vec<u8> {
                    let mut res = vec![
                        $fix_eip2537_fp(g[..g.len() / 2].to_vec()),
                        $fix_eip2537_fp(g[g.len() / 2..].to_vec()),
                    ]
                    .concat();

                    if g == vec![0; g.len()] {
                        res[0] |= 0x40;
                    }

                    return res;
                }

                pub fn fix_eip2537_sum_input(input: Vec<u8>) -> Vec<u8> {
                    vec![
                        vec![0u8],
                        fix_eip2537_g(input[..input.len() / 2].to_vec()),
                        vec![0u8],
                        fix_eip2537_g(input[input.len() / 2..].to_vec()),
                    ]
                    .concat()
                }

                pub fn fix_eip2537_mul_input(input: Vec<u8>) -> Vec<u8> {
                    vec![
                        fix_eip2537_g(input[..(input.len() - 32)].to_vec()),
                        input[(input.len() - 32)..].to_vec().into_iter().rev().collect(),
                    ]
                    .concat()
                }

                pub fn cmp_output_g(output: &str, res: Vec<u8>) {
                    let bytes_output = fix_eip2537_g(hex::decode(output).unwrap());
                    assert_eq!(res, bytes_output);
                }
            }
        };
    }

    fix_eip2537_input!(fix_eip2537_g1_namespace, fix_eip2537_fp);
    use fix_eip2537_g1_namespace::cmp_output_g as cmp_output_g1;
    use fix_eip2537_g1_namespace::fix_eip2537_g as fix_eip2537_g1;
    use fix_eip2537_g1_namespace::fix_eip2537_mul_input as fix_eip2537_mul_g1_input;
    use fix_eip2537_g1_namespace::fix_eip2537_sum_input as fix_eip2537_sum_g1_input;

    fix_eip2537_input!(fix_eip2537_g2_namespace, fix_eip2537_fp2);
    use fix_eip2537_g2_namespace::cmp_output_g as cmp_output_g2;
    use fix_eip2537_g2_namespace::fix_eip2537_g as fix_eip2537_g2;
    use fix_eip2537_g2_namespace::fix_eip2537_mul_input as fix_eip2537_mul_g2_input;
    use fix_eip2537_g2_namespace::fix_eip2537_sum_input as fix_eip2537_sum_g2_input;

    fn check_pairing_res(output: &str, res: u64) {
        if output == "0000000000000000000000000000000000000000000000000000000000000000" {
            assert_eq!(res, 2);
        } else if output == "0000000000000000000000000000000000000000000000000000000000000001" {
            assert_eq!(res, 0);
        } else {
            assert_eq!(res, 1);
        }
    }

    fn error_check(output: &str, res: u64) {
        if !output.contains("padded BE encoding are NOT zeroes") {
            assert_eq!(res, 1)
        }
    }

    macro_rules! run_bls12381_fn_raw {
        ($fn_name_raw:ident, $fn_name_return_value_only:ident, $bls_fn_name:ident) => {
            #[allow(unused)]
            fn $fn_name_raw(input: MemSlice, logic: &mut TestVMLogic) -> Vec<u8> {
                let res = logic.$bls_fn_name(input.len, input.ptr, 0).unwrap();
                assert_eq!(res, 0);
                logic.registers().get_for_free(0).unwrap().to_vec()
            }

            #[allow(unused)]
            fn $fn_name_return_value_only(input: MemSlice, logic: &mut TestVMLogic) -> u64 {
                logic.$bls_fn_name(input.len, input.ptr, 0).unwrap()
            }
        };
    }

    run_bls12381_fn_raw!(run_map_fp_to_g1, map_fp_to_g1_return_value, bls12381_map_fp_to_g1);
    run_bls12381_fn_raw!(run_map_fp2_to_g2, map_fp2tog2_return_value, bls12381_map_fp2_to_g2);
    run_bls12381_fn_raw!(run_sum_g1, sum_g1_return_value, bls12381_p1_sum);
    run_bls12381_fn_raw!(run_sum_g2, sum_g2_return_value, bls12381_p2_sum);
    run_bls12381_fn_raw!(run_multiexp_g1, multiexp_g1_return_value, bls12381_p1_multiexp);
    run_bls12381_fn_raw!(run_multiexp_g2, multiexp_g2_return_value, bls12381_p2_multiexp);
    run_bls12381_fn_raw!(decompress_g1, decompress_g1_return_value, bls12381_p1_decompress);
    run_bls12381_fn_raw!(decompress_g2, decompress_g2_return_value, bls12381_p2_decompress);
    fn run_pairing_check_raw(input: MemSlice, logic: &mut TestVMLogic) -> u64 {
        logic.bls12381_pairing_check(input.len, input.ptr).unwrap()
    }

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/pairing.csv",
        test_bls12381_pairing_test_vectors,
        384,
        fix_eip2537_pairing_input,
        run_pairing_check_raw,
        check_pairing_res
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/fp_to_g1.csv",
        test_bls12381_fp_to_g1_test_vectors,
        64,
        fix_eip2537_fp,
        run_map_fp_to_g1,
        cmp_output_g1
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/fp2_to_g2.csv",
        test_bls12381_fp2_to_g2_test_vectors,
        128,
        fix_eip2537_fp2,
        run_map_fp2_to_g2,
        cmp_output_g2
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/g1_add.csv",
        test_bls12381_g1_add_test_vectors,
        256,
        fix_eip2537_sum_g1_input,
        run_sum_g1,
        cmp_output_g1
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/g2_add.csv",
        test_bls12381_g2_add_test_vectors,
        512,
        fix_eip2537_sum_g2_input,
        run_sum_g2,
        cmp_output_g2
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/g1_mul.csv",
        test_bls12381_g1_mul_test_vectors,
        160,
        fix_eip2537_mul_g1_input,
        run_multiexp_g1,
        cmp_output_g1
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/g2_mul.csv",
        test_bls12381_g2_mul_test_vectors,
        288,
        fix_eip2537_mul_g2_input,
        run_multiexp_g2,
        cmp_output_g2
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/g1_multiexp.csv",
        test_bls12381_g1_multiexp_test_vectors,
        160,
        fix_eip2537_mul_g1_input,
        run_multiexp_g1,
        cmp_output_g1
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/g2_multiexp.csv",
        test_bls12381_g2_multiexp_test_vectors,
        288,
        fix_eip2537_mul_g2_input,
        run_multiexp_g2,
        cmp_output_g2
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/pairing_error.csv",
        test_bls12381_pairing_error_test_vectors,
        384,
        fix_eip2537_pairing_input,
        run_pairing_check_raw,
        check_pairing_res
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/multiexp_g1_error.csv",
        test_bls12381_g1_multiexp_error_test_vectors,
        160,
        fix_eip2537_mul_g1_input,
        multiexp_g1_return_value,
        error_check
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/multiexp_g2_error.csv",
        test_bls12381_g2_multiexp_error_test_vectors,
        288,
        fix_eip2537_mul_g2_input,
        multiexp_g2_return_value,
        error_check
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/fp_to_g1_error.csv",
        test_bls12381_fp_to_g1_error_test_vectors,
        64,
        fix_eip2537_fp,
        map_fp_to_g1_return_value,
        error_check
    );

    eip2537_tests!(
        "src/logic/tests/bls12381_test_vectors/fp2_to_g2_error.csv",
        test_bls12381_fp2_to_g2_error_test_vectors,
        128,
        fix_eip2537_fp2,
        map_fp2tog2_return_value,
        error_check
    );
}

mod tests {
    use crate::logic::tests::vm_logic_builder::{TestVMLogic, VMLogicBuilder};
    use amcl::bls381::big::Big;
    use amcl::bls381::bls381::core::deserialize_g1;
    use amcl::bls381::ecp::ECP;
    use amcl::rand::RAND;
    use amcl::bls381::bls381::utils::{subgroup_check_g1, serialize_uncompressed_g1};

    fn get_random_g1_point(rnd: &mut RAND) -> ECP {
        let r: Big = Big::random(rnd);
        let g: ECP = ECP::generator();

        g.mul(&r)
    }

    fn get_random_curve_point(rnd: &mut RAND) -> ECP {
        let mut r: Big = Big::random(rnd);
        r.mod2m(381);
        let mut p: ECP = ECP::new_big(&r);

        while p.is_infinity() {
            r = Big::random(rnd);
            r.mod2m(381);
            p = ECP::new_big(&r);
        }

        p
    }

    fn get_random_not_g1_curve_point(rnd: &mut RAND) -> ECP {
        let mut r: Big = Big::random(rnd);
        r.mod2m(381);
        let mut p: ECP = ECP::new_big(&r);

        while p.is_infinity() || subgroup_check_g1(&p) {
            r = Big::random(rnd);
            r.mod2m(381);
            p = ECP::new_big(&r);
        }

        p
    }

    fn get_g1_sum(p: &[u8], q: &[u8], logic: &mut TestVMLogic) -> Vec<u8> {
        let mut buffer = vec![vec![0], p.to_vec(), vec![0], q.to_vec()];

        println!("{:?}", buffer);

        let input = logic.internal_mem_write(buffer.concat().as_slice());
        let res = logic.bls12381_g1_sum(input.len, input.ptr, 0).unwrap();
        assert_eq!(res, 0);
        logic.registers().get_for_free(0).unwrap().to_vec()
    }

    #[test]
    fn test_bls12381_g1_sum_edge_cases() {
        let mut logic_builder = VMLogicBuilder::default();
        let mut logic = logic_builder.build();

        // 0 + 0
        let mut zero: [u8; 97] = [0; 97];
        zero[1] = 64;
        let buffer: [[u8; 97]; 2] = [zero; 2];

        let input = logic.internal_mem_write(buffer.concat().as_slice());

        let res = logic.bls12381_g1_sum(input.len, input.ptr, 0).unwrap();
        assert_eq!(res, 0);
        let got = logic.registers().get_for_free(0).unwrap();
        assert_eq!(&zero[1..97], got);


        // 0 + P = P + 0 = P
        let mut rnd: RAND = RAND::new();
        for _ in 0..10 {
            let p = get_random_g1_point(&mut rnd);
            let p_ser = serialize_uncompressed_g1(&p);

            let mut buffer = vec![vec![0], p_ser.to_vec(), zero.to_vec()];

            let mut input = logic.internal_mem_write(buffer.concat().as_slice());

            let res = logic.bls12381_g1_sum(input.len, input.ptr, 0).unwrap();
            assert_eq!(res, 0);
            let got = logic.registers().get_for_free(0).unwrap();
            assert_eq!(&p_ser, got);

            buffer = vec![zero.to_vec(), vec![0], p_ser.to_vec()];
            input = logic.internal_mem_write(buffer.concat().as_slice());

            let res = logic.bls12381_g1_sum(input.len, input.ptr, 0).unwrap();
            assert_eq!(res, 0);
            let got = logic.registers().get_for_free(0).unwrap();
            assert_eq!(&p_ser, got);
        }
    }

    #[test]
    fn test_bls12381_g1_sum() {
        let mut logic_builder = VMLogicBuilder::default();
        let mut logic = logic_builder.build();

        let mut rnd: RAND = RAND::new();
        for _ in 0..100 {
            let mut p = get_random_curve_point(&mut rnd);
            let p_ser = serialize_uncompressed_g1(&p);

            let q = get_random_curve_point(&mut rnd);
            let q_ser = serialize_uncompressed_g1(&q);

            let mut buffer = vec![vec![0], p_ser.to_vec(), vec![0], q_ser.to_vec()];
            let input = logic.internal_mem_write(buffer.concat().as_slice());
            let res = logic.bls12381_g1_sum(input.len, input.ptr, 0).unwrap();
            assert_eq!(res, 0);
            let got1 = logic.registers().get_for_free(0).unwrap().to_vec();

            // P + Q = Q + P
            buffer = vec![vec![0], q_ser.to_vec(), vec![0], p_ser.to_vec()];
            let input = logic.internal_mem_write(buffer.concat().as_slice());
            let res = logic.bls12381_g1_sum(input.len, input.ptr, 0).unwrap();
            assert_eq!(res, 0);
            let got2 = logic.registers().get_for_free(0).unwrap().to_vec();
            assert_eq!(got1, got2);

            // compare with library results
            p.add(&q);
            let library_sum = serialize_uncompressed_g1(&p);

            assert_eq!(library_sum.to_vec(), got1);
        }

        // generate points from G1
        for _ in 0..100 {
            let p = get_random_g1_point(&mut rnd);
            let p_ser = serialize_uncompressed_g1(&p);

            let q = get_random_g1_point(&mut rnd);
            let q_ser = serialize_uncompressed_g1(&q);

            let buffer = vec![vec![0], p_ser.to_vec(), vec![0], q_ser.to_vec()];
            let input = logic.internal_mem_write(buffer.concat().as_slice());

            let res = logic.bls12381_g1_sum(input.len, input.ptr, 0).unwrap();
            assert_eq!(res, 0);

            let got1 = logic.registers().get_for_free(0).unwrap().to_vec();

            let result_point = deserialize_g1(&got1).unwrap();
            assert!(subgroup_check_g1(&result_point));
        }
    }

    #[test]
    fn test_bls12381_g1_sum_not_g1_points() {
        let mut logic_builder = VMLogicBuilder::default();
        let mut logic = logic_builder.build();

        let mut rnd: RAND = RAND::new();
        rnd.clean();
        let mut raw : [u8;100]=[0;100];
        for i in 0..100 {raw[i]=i as u8}

        rnd.seed(100,&raw);

        //points not from G1
        for _ in 0..100 {
            let mut p = get_random_not_g1_curve_point(&mut rnd);
            let p_ser = serialize_uncompressed_g1(&p);

            let q = get_random_not_g1_curve_point(&mut rnd);
            let q_ser = serialize_uncompressed_g1(&q);

            // P + Q = Q + P
            let got1 = get_g1_sum(&p_ser, &q_ser, &mut logic);
            let got2 = get_g1_sum(&q_ser, &p_ser, &mut logic);
            assert_eq!(got1, got2);

            // compare with library results
            p.add(&q);
            let library_sum = serialize_uncompressed_g1(&p);

            assert_eq!(library_sum.to_vec(), got1);
        }
    }
}
